/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

//! 元数据导入命令实现（基于 API）
//!
//! 通过 HTTP API 导入元数据

use anyhow::{anyhow, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::cli::client::ApiClient;
use crate::cli::dto::{PackageDto, TrackingDto};
use crate::cli::formatter::format_datetime_local;
use crate::cli::parser::ImportAction;
use crate::snapshot::types::RepositorySnapshot;
use crate::snapshot::types::{
    ChangeStats, CommitEntry, FileEntry, IssueEntry, SnapshotOrigin, SpecEntry,
};
use chrono::{DateTime, Utc};
use serde_json::Value;

/// 导入响应
#[derive(Debug, Serialize, Deserialize)]
struct ImportResponse {
    snapshot_id: String,
    tracking_id: i32,
    file_count: usize,
    imported_at: chrono::DateTime<chrono::Utc>,
}

/// API 响应包装
#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T> {
    code: u16,
    message: String,
    data: Option<T>,
}

/// 导入请求
#[derive(Debug, Serialize, Deserialize)]
struct ImportRequest {
    tracking_id: i32,
    snapshot: RepositorySnapshot,
}

/// 列表响应
#[derive(Debug, Serialize, Deserialize)]
struct ListResponse<T> {
    items: Vec<T>,
    total: usize,
}

/// 执行导入命令
pub async fn execute(api_client: &ApiClient, action: ImportAction) -> Result<()> {
    match action {
        ImportAction::Metadata { file, tracking_id } => {
            let path = PathBuf::from(file);
            import_single_file(api_client, &path, Some(tracking_id)).await
        }
        ImportAction::Batch { files, tracking_id } => {
            let paths: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
            import_batch_files(api_client, paths, tracking_id).await
        }
    }
}

/// 从 JSON 内容中提取 repo/branch/level
fn extract_repo_branch_origin_from_json(content: &str) -> Result<(String, String, SnapshotOrigin)> {
    let root: Value =
        serde_json::from_str(content).map_err(|e| anyhow!("解析 JSON 失败: {}", e))?;

    let repo = root
        .get("repo")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("JSON 文件中未找到 'repo' 字段"))?
        .to_string();
    let branch = root
        .get("branch")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("JSON 文件中未找到 'branch' 字段"))?
        .to_string();
    let origin = match root.get("level").and_then(|v| v.as_str()) {
        Some("l2") => SnapshotOrigin::L2,
        Some("l1") => SnapshotOrigin::L1,
        _ => SnapshotOrigin::Unknown,
    };
    Ok((repo, branch, origin))
}

/// 根据 repo + branch 解析 tracking_id
async fn resolve_tracking_id_from_repo_branch(
    api_client: &ApiClient,
    repo: &str,
    branch: &str,
    origin: SnapshotOrigin,
) -> Result<i32> {
    let packages: Vec<PackageDto> = api_client
        .get("/packages")
        .await
        .map_err(|e| anyhow!("查询 package 列表失败: {}", e))?;

    let package = packages
        .iter()
        .find(|p| p.name == repo)
        .ok_or_else(|| anyhow!("未找到名称为 '{}' 的 package", repo))?;

    let package_id = package.id;
    let trackings = fetch_trackings(api_client, package_id).await?;
    if trackings.is_empty() {
        return Err(anyhow!(
            "package '{}' (ID: {}) 没有关联的 tracking 配置，请先创建",
            repo,
            package_id
        ));
    }

    let branch_candidates = build_branch_candidates(branch);
    let mut candidates: Vec<&TrackingDto> = match origin {
        SnapshotOrigin::L2 => trackings
            .iter()
            .filter(|t| branch_candidates.iter().any(|b| t.l2_branch == *b))
            .collect(),
        SnapshotOrigin::L1 => trackings
            .iter()
            .filter(|t| branch_candidates.iter().any(|b| t.l1_branch == *b))
            .collect(),
        SnapshotOrigin::Unknown => Vec::new(),
    };

    if candidates.is_empty() {
        candidates = trackings
            .iter()
            .filter(|t| {
                branch_candidates
                    .iter()
                    .any(|b| t.l1_branch == *b || t.l2_branch == *b)
            })
            .collect();
    }

    let tracking = candidates
        .iter()
        .find(|t| t.tracking_status == "active")
        .or_else(|| candidates.first())
        .ok_or_else(|| anyhow!("未找到匹配分支 '{}' 的 tracking", branch))?;

    Ok(tracking.id)
}

fn build_branch_candidates(branch: &str) -> Vec<String> {
    let mut candidates = vec![branch.to_string()];
    let lower = branch.to_lowercase();
    if let Some(stripped) = lower.strip_prefix("ctyunos-") {
        if stripped != lower {
            candidates.push(stripped.to_string());
            if let Some((prefix, rest)) = stripped.split_once('-') {
                if !rest.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()) {
                    candidates.push(rest.to_string());
                }
            }
        }
    }
    if let Some(stripped) = lower.strip_prefix("ctyunos_") {
        if stripped != lower {
            candidates.push(stripped.to_string());
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

async fn fetch_trackings(api_client: &ApiClient, package_id: i32) -> Result<Vec<TrackingDto>> {
    let mut trackings = Vec::new();
    let mut page = 1;
    let page_size = 100;
    loop {
        let query = format!(
            "?page={}&page_size={}&package_id={}",
            page, page_size, package_id
        );
        let response: ApiResponse<ListResponse<TrackingDto>> = api_client
            .get(&format!("/tracking{}", query))
            .await
            .map_err(|e| anyhow!("查询 tracking 配置失败: {}", e))?;
        let items = response.data.map(|data| data.items).unwrap_or_default();
        let items_len = items.len();
        trackings.extend(items);
        if items_len < page_size as usize {
            break;
        }
        page += 1;
    }
    Ok(trackings)
}

/// 导入单个文件
async fn import_single_file(
    api_client: &ApiClient,
    file: &PathBuf,
    tracking_id: Option<i32>,
) -> Result<()> {
    println!("正在导入文件: {}", file.display().to_string().cyan());

    // 检查文件是否存在
    if !file.exists() {
        println!("{} 文件不存在: {}", "✗".red().bold(), file.display());
        anyhow::bail!("文件不存在");
    }

    let content = fs::read_to_string(file)?;
    let (resolved_tracking_id, snapshot) = if let Some(id) = tracking_id {
        let snapshot = parse_snapshot_or_convert(&content, id)?;
        (id, snapshot)
    } else if let Ok(snapshot) = serde_json::from_str::<RepositorySnapshot>(&content) {
        if snapshot.tracking_id > 0 {
            (snapshot.tracking_id, snapshot)
        } else {
            let (repo, branch, origin) = extract_repo_branch_origin_from_json(&content)?;
            let resolved =
                resolve_tracking_id_from_repo_branch(api_client, &repo, &branch, origin).await?;
            let snapshot = parse_snapshot_or_convert(&content, resolved)?;
            (resolved, snapshot)
        }
    } else {
        let (repo, branch, origin) = extract_repo_branch_origin_from_json(&content)?;
        let resolved =
            resolve_tracking_id_from_repo_branch(api_client, &repo, &branch, origin).await?;
        let snapshot = parse_snapshot_or_convert(&content, resolved)?;
        (resolved, snapshot)
    };

    // 确定导入端点（根据快照来源）
    let endpoint = match snapshot.origin {
        SnapshotOrigin::L1 => "/metadata/l1",
        SnapshotOrigin::L2 => "/metadata/l2",
        SnapshotOrigin::Unknown => {
            // 默认使用 L1
            println!("{}", "警告: 快照来源未知，默认使用 L1 端点".yellow());
            "/metadata/l1"
        }
    };

    println!("  快照来源: {:?}", snapshot.origin);
    println!("  文件数量: {}", snapshot.files.len());
    println!("  提交数量: {}", snapshot.commits.len());
    println!("  问题数量: {}", snapshot.issues.len());
    println!();

    // 构建请求
    let request = ImportRequest {
        tracking_id: resolved_tracking_id,
        snapshot,
    };

    // 发送导入请求
    match api_client
        .post::<_, ApiResponse<ImportResponse>>(endpoint, &request)
        .await
    {
        Ok(response) => {
            let data = response
                .data
                .ok_or_else(|| anyhow::anyhow!("API 响应缺少 data 字段"))?;
            println!("{} 导入成功", "✓".green().bold());
            println!("  快照 ID: {}", data.snapshot_id.cyan());
            println!("  跟踪配置 ID: {}", data.tracking_id);
            println!("  文件数量: {}", data.file_count);
            println!("  导入时间: {}", format_datetime_local(&data.imported_at));
            Ok(())
        }
        Err(e) => {
            println!("{} 导入失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 批量导入文件
/// tracking_id 将从每个 JSON 文件的 repo 字段自动解析
async fn import_batch_files(
    api_client: &ApiClient,
    files: Vec<PathBuf>,
    tracking_id: Option<i32>,
) -> Result<()> {
    println!("开始批量导入 {} 个文件", files.len());
    println!();

    let mut success_count = 0;
    let mut failed_count = 0;
    let mut total_files = 0;

    for (i, file) in files.iter().enumerate() {
        println!("[{}/{}] 导入文件: {}", i + 1, files.len(), file.display());

        let content = fs::read_to_string(file)?;
        let resolved = if let Some(tracking_id) = tracking_id {
            Some(tracking_id)
        } else if let Ok(snapshot) = serde_json::from_str::<RepositorySnapshot>(&content) {
            if snapshot.tracking_id > 0 {
                Some(snapshot.tracking_id)
            } else {
                None
            }
        } else {
            None
        };

        match resolved {
            Some(tracking_id) => {
                println!("  使用 tracking_id: {}", tracking_id);
                match import_single_file(api_client, file, Some(tracking_id)).await {
                    Ok(_) => {
                        success_count += 1;
                        // 读取文件获取文件数量
                        if let Ok(content) = fs::read_to_string(file) {
                            if let Ok(snapshot) =
                                serde_json::from_str::<RepositorySnapshot>(&content)
                            {
                                total_files += snapshot.files.len();
                            }
                        }
                    }
                    Err(e) => {
                        println!("  导入失败: {}", e);
                        failed_count += 1;
                    }
                }
            }
            None => {
                let (repo, branch, origin) = extract_repo_branch_origin_from_json(&content)?;
                match resolve_tracking_id_from_repo_branch(api_client, &repo, &branch, origin).await
                {
                    Ok(tracking_id) => {
                        println!("  解析到 tracking_id: {}", tracking_id);
                        match import_single_file(api_client, file, Some(tracking_id)).await {
                            Ok(_) => {
                                success_count += 1;
                                if let Ok(content) = fs::read_to_string(file) {
                                    if let Ok(snapshot) =
                                        serde_json::from_str::<RepositorySnapshot>(&content)
                                    {
                                        total_files += snapshot.files.len();
                                    }
                                }
                            }
                            Err(e) => {
                                println!("  导入失败: {}", e);
                                failed_count += 1;
                            }
                        }
                    }
                    Err(e) => {
                        println!("  无法解析 tracking_id: {}", e);
                        failed_count += 1;
                    }
                }
            }
        }
        println!();
    }

    println!("{}", "批量导入完成:".bold());
    println!("  成功: {}", success_count.to_string().green());
    println!("  失败: {}", failed_count.to_string().red());
    println!("  总文件数: {}", total_files);

    if failed_count > 0 {
        anyhow::bail!("{} 个文件导入失败", failed_count);
    }

    Ok(())
}

/// 尝试解析 RepositorySnapshot；失败则按通用采集 JSON 转换为 RepositorySnapshot
fn parse_snapshot_or_convert(
    content: &str,
    tracking_id: i32,
) -> anyhow::Result<RepositorySnapshot> {
    // 1) 优先当作 RepositorySnapshot 解析
    if let Ok(snapshot) = serde_json::from_str::<RepositorySnapshot>(content) {
        return Ok(snapshot);
    }

    // 2) 解析为通用 JSON，转换为 RepositorySnapshot
    let root: Value = serde_json::from_str(content)?;

    // 读取顶层字段：level/collected_at（可选）
    let level = root.get("level").and_then(|v| v.as_str()).unwrap_or("l1");
    let origin = match level {
        "l2" => SnapshotOrigin::L2,
        // 将 l0 也归并到 L1，以便走同一导入端点
        "l0" | "l1" => SnapshotOrigin::L1,
        _ => SnapshotOrigin::L1,
    };

    let generated_at = root
        .get("collected_at")
        .and_then(|v| v.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    // commits 列表
    let commits_arr = root
        .get("commits")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // issues 列表（可选）
    let issues_arr = root
        .get("issues")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // spec 信息（可选）
    let spec_entry = root.get("spec").and_then(|v| {
        let path = v
            .get("path")
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string();
        let version = v
            .get("version")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string());
        let release = v
            .get("release")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string());
        let content_base64 = v
            .get("content_base64")
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string();
        let sha256 = v
            .get("sha256")
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string();

        if !path.is_empty() && !content_base64.is_empty() {
            Some(SpecEntry {
                path,
                version,
                release,
                content_base64,
                sha256,
            })
        } else {
            None
        }
    });

    // files 列表（可选）
    let files_arr = root
        .get("files")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // 将通用 commit JSON 转换为 CommitEntry
    let commits: Vec<CommitEntry> = commits_arr
        .into_iter()
        .map(|c| {
            // 字段兼容映射
            let sha = c
                .get("sha")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let message = c
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let title = c
                .get("title")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| message.lines().next().unwrap_or("").to_string());
            let author = c
                .get("author")
                .and_then(|v| v.as_str())
                .or_else(|| c.get("author_name").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string();
            let authored_at = c
                .get("authored_at")
                .and_then(|v| v.as_str())
                .or_else(|| c.get("author_date").and_then(|v| v.as_str()))
                .or_else(|| c.get("date").and_then(|v| v.as_str()))
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);
            let url = c.get("url").and_then(|v| v.as_str()).map(|s| s.to_string());
            let additions = c.get("additions").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let deletions = c.get("deletions").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let files_changed = c
                .get("files_changed")
                .and_then(|v| v.as_i64())
                .or_else(|| c.get("files_changed_count").and_then(|v| v.as_i64()))
                .unwrap_or(0) as i32;
            let stats = ChangeStats {
                additions,
                deletions,
                files_changed,
            };
            let primary_change_type = c
                .get("primary_change_type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let cve_list = c
                .get("cve_list")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|e| e.as_str().map(|s| s.to_string()))
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();

            CommitEntry {
                sha,
                title,
                message,
                author,
                authored_at,
                url,
                stats,
                primary_change_type,
                cve_list,
            }
        })
        .collect();

    // 将通用 issue JSON 转换为 IssueEntry
    let issues: Vec<IssueEntry> = issues_arr
        .into_iter()
        .map(|i| {
            let number = i
                .get("number")
                .and_then(|v| {
                    v.as_i64()
                        .map(|n| n.to_string())
                        .or_else(|| v.as_str().map(|s| s.to_string()))
                })
                .unwrap_or_default();
            let title = i
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let state = i
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let author = i
                .get("author")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let labels = i
                .get("labels")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|e| e.as_str().map(|s| s.to_string()))
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();
            let updated_at = i
                .get("updated_at")
                .and_then(|v| v.as_str())
                .or_else(|| i.get("date").and_then(|v| v.as_str()))
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);

            IssueEntry {
                number,
                title,
                state,
                author,
                labels,
                updated_at,
            }
        })
        .collect();

    // 将通用 file JSON 转换为 FileEntry
    let files: Vec<FileEntry> = files_arr
        .into_iter()
        .filter_map(|f| {
            let path = f.get("path").and_then(|v| v.as_str())?.to_string();
            let sha256 = f
                .get("sha256")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let size = f.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
            let is_binary = f
                .get("is_binary")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            Some(FileEntry {
                path,
                sha256,
                size,
                is_binary,
            })
        })
        .collect();

    Ok(RepositorySnapshot {
        tracking_id,
        generated_at,
        origin,
        files,
        spec: spec_entry,
        commits,
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::client::ClientConfig;
    use mockito::Server;
    use std::io::Write;
    use tempfile::NamedTempFile;

    async fn setup_test_server() -> (mockito::ServerGuard, ApiClient) {
        let server = Server::new_async().await;
        let config = ClientConfig {
            server_url: server.url(),
            auth_token: Some("test_token".to_string()),
            timeout: 30,
            verify_ssl: true,
        };
        let client = ApiClient::new(config).unwrap();
        (server, client)
    }

    fn create_test_snapshot_json() -> String {
        serde_json::json!({
            "tracking_id": 1,
            "generated_at": "2024-01-01T00:00:00Z",
            "origin": "L1",
            "files": [],
            "commits": [
                {
                    "sha": "abc123",
                    "title": "Test commit",
                    "message": "Test commit message",
                    "author": "Test Author",
                    "authored_at": "2024-01-01T00:00:00Z",
                    "stats": {
                        "additions": 10,
                        "deletions": 5,
                        "files_changed": 2
                    },
                    "cve_list": []
                }
            ],
            "issues": []
        })
        .to_string()
    }

    #[test]
    fn test_parse_snapshot_or_convert_valid_snapshot() {
        let json = create_test_snapshot_json();
        let result = parse_snapshot_or_convert(&json, 1);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
        let snapshot = result.unwrap();
        assert_eq!(snapshot.tracking_id, 1);
        assert_eq!(snapshot.commits.len(), 1);
    }

    #[test]
    fn test_parse_snapshot_or_convert_generic_json() {
        let json = serde_json::json!({
            "level": "l1",
            "collected_at": "2024-01-01T00:00:00Z",
            "commits": [
                {
                    "sha": "def456",
                    "message": "Generic commit",
                    "author": "Generic Author",
                    "date": "2024-01-01T00:00:00Z",
                    "additions": 5,
                    "deletions": 3,
                    "files_changed": 1
                }
            ]
        })
        .to_string();

        let result = parse_snapshot_or_convert(&json, 2);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
        let snapshot = result.unwrap();
        assert_eq!(snapshot.tracking_id, 2);
        assert_eq!(snapshot.commits.len(), 1);
        assert_eq!(snapshot.commits[0].sha, "def456");
    }

    #[test]
    fn test_parse_snapshot_or_convert_generic_spec_entry_present() {
        let json = serde_json::json!({
            "level": "l0",
            "collected_at": "2024-01-01T00:00:00Z",
            "spec": {
                "path": "test.spec",
                "version": "1.0.0",
                "release": "1",
                "content_base64": "dGVzdA==",
                "sha256": "spec-sha"
            },
            "files": [
                {"path": "a.patch", "sha256": "x", "size": 1, "is_binary": false}
            ],
            "commits": [],
            "issues": []
        })
        .to_string();

        let snapshot = parse_snapshot_or_convert(&json, 99).unwrap();
        assert_eq!(snapshot.origin, SnapshotOrigin::L1);
        assert!(snapshot.spec.is_some());
        let spec = snapshot.spec.unwrap();
        assert_eq!(spec.path, "test.spec");
        assert_eq!(spec.version.as_deref(), Some("1.0.0"));
        assert_eq!(spec.release.as_deref(), Some("1"));
        assert_eq!(spec.content_base64, "dGVzdA==");
        assert_eq!(spec.sha256, "spec-sha");
    }

    #[test]
    fn test_parse_snapshot_or_convert_generic_spec_entry_missing_content() {
        let json = serde_json::json!({
            "level": "l1",
            "collected_at": "2024-01-01T00:00:00Z",
            "spec": {
                "path": "test.spec",
                "content_base64": "",
                "sha256": "spec-sha"
            },
            "commits": [],
            "issues": []
        })
        .to_string();

        let snapshot = parse_snapshot_or_convert(&json, 99).unwrap();
        assert!(snapshot.spec.is_none());
    }

    #[tokio::test]
    async fn test_import_single_file() {
        let (mut server, client) = setup_test_server().await;

        // 创建临时文件
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write_all(create_test_snapshot_json().as_bytes())
            .unwrap();
        temp_file.flush().unwrap();

        let mock = server
            .mock("POST", "/api/metadata/l1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "code": 200,
                    "message": "success",
                    "data": {
                        "snapshot_id": "snap-test-123",
                        "tracking_id": 1,
                        "file_count": 10,
                        "imported_at": "2024-01-01T00:00:00Z"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = import_single_file(&client, &temp_file.path().to_path_buf(), Some(1)).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_import_single_file_unknown_origin_uses_l1_endpoint() {
        let (mut server, client) = setup_test_server().await;

        let json = serde_json::json!({
            "tracking_id": 1,
            "generated_at": "2024-01-01T00:00:00Z",
            "origin": "Unknown",
            "files": [],
            "spec": null,
            "commits": [],
            "issues": []
        })
        .to_string();

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(json.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let mock = server
            .mock("POST", "/api/metadata/l1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "code": 200,
                    "message": "success",
                    "data": {
                        "snapshot_id": "snap-unknown-origin",
                        "tracking_id": 1,
                        "file_count": 0,
                        "imported_at": "2024-01-01T00:00:00Z"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = import_single_file(&client, &temp_file.path().to_path_buf(), Some(1)).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_import_single_file_l2_endpoint() {
        let (mut server, client) = setup_test_server().await;

        let json = serde_json::json!({
            "level": "l2",
            "collected_at": "2024-01-01T00:00:00Z",
            "commits": [],
            "issues": []
        })
        .to_string();

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(json.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let mock = server
            .mock("POST", "/api/metadata/l2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "code": 200,
                    "message": "success",
                    "data": {
                        "snapshot_id": "snap-l2",
                        "tracking_id": 1,
                        "file_count": 0,
                        "imported_at": "2024-01-01T00:00:00Z"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = import_single_file(&client, &temp_file.path().to_path_buf(), Some(1)).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_import_single_file_api_missing_data() {
        let (mut server, client) = setup_test_server().await;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write_all(create_test_snapshot_json().as_bytes())
            .unwrap();
        temp_file.flush().unwrap();

        let mock = server
            .mock("POST", "/api/metadata/l1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "code": 200,
                    "message": "success",
                    "data": null
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = import_single_file(&client, &temp_file.path().to_path_buf(), Some(1)).await;
        assert!(result.is_err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_import_single_file_api_error() {
        let (mut server, client) = setup_test_server().await;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write_all(create_test_snapshot_json().as_bytes())
            .unwrap();
        temp_file.flush().unwrap();

        let mock = server
            .mock("POST", "/api/metadata/l1")
            .with_status(500)
            .with_header("content-type", "text/plain")
            .with_body("server error")
            .create_async()
            .await;

        let result = import_single_file(&client, &temp_file.path().to_path_buf(), Some(1)).await;
        assert!(result.is_err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_import_single_file_not_exists() {
        let (_server, client) = setup_test_server().await;
        let non_existent = PathBuf::from("/nonexistent/file.json");
        let result = import_single_file(&client, &non_existent, Some(1)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolve_tracking_id_from_repo_branch() {
        let (mut server, client) = setup_test_server().await;

        // Mock packages endpoint
        let packages_mock_1 = server
            .mock("GET", "/api/packages")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!([
                    {
                        "id": 10,
                        "name": "test-package",
                        "level": 1,
                        "sync_interval_hours": 24,
                        "l0_repo_url": "https://github.com/test/test",
                        "description": "Test package",
                        "created_at": "2024-01-01T00:00:00Z",
                        "updated_at": "2024-01-01T00:00:00Z"
                    }
                ])
                .to_string(),
            )
            .create_async()
            .await;

        // Mock tracking endpoint
        let tracking_mock = server
            .mock("GET", "/api/tracking?page=1&page_size=100&package_id=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "code": 200,
                    "message": "success",
                    "data": {
                        "items": [
                            {
                                "id": 100,
                                "package_id": 10,
                                "distro_id": 1,
                                "l1_repo_owner": "test-owner",
                                "l1_repo_name": "test-repo",
                                "l1_branch": "main",
                                "l2_branch": "openeuler",
                                "l2_repo_path": "/path/to/repo",
                                "tracking_status": "active",
                                "last_sync_time": null,
                                "last_l1_commit_sha": null,
                                "last_l2_commit_sha": null,
                                "created_at": "2024-01-01T00:00:00Z",
                                "updated_at": "2024-01-01T00:00:00Z"
                            }
                        ],
                        "total": 1
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = resolve_tracking_id_from_repo_branch(
            &client,
            "test-package",
            "main",
            SnapshotOrigin::L1,
        )
        .await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        assert_eq!(result.unwrap(), 100);
        packages_mock_1.assert_async().await;
        tracking_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_resolve_tracking_id_from_repo_branch_ctyunos_prefix() {
        let (mut server, client) = setup_test_server().await;

        let packages_mock = server
            .mock("GET", "/api/packages")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!([
                    {
                        "id": 11,
                        "name": "ctyunos-package",
                        "level": 1,
                        "sync_interval_hours": 24,
                        "l0_repo_url": "https://github.com/test/test",
                        "description": "Test package",
                        "created_at": "2024-01-01T00:00:00Z",
                        "updated_at": "2024-01-01T00:00:00Z"
                    }
                ])
                .to_string(),
            )
            .create_async()
            .await;

        let tracking_mock = server
            .mock("GET", "/api/tracking?page=1&page_size=100&package_id=11")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "code": 200,
                    "message": "success",
                    "data": {
                        "items": [
                            {
                                "id": 101,
                                "package_id": 11,
                                "distro_id": 1,
                                "l1_repo_owner": "test-owner",
                                "l1_repo_name": "test-repo",
                                "l1_branch": "main",
                                "l2_branch": "2.0.1",
                                "l2_repo_path": "/path/to/repo",
                                "tracking_status": "active",
                                "last_sync_time": null,
                                "last_l1_commit_sha": null,
                                "last_l2_commit_sha": null,
                                "created_at": "2024-01-01T00:00:00Z",
                                "updated_at": "2024-01-01T00:00:00Z"
                            }
                        ],
                        "total": 1
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = resolve_tracking_id_from_repo_branch(
            &client,
            "ctyunos-package",
            "ctyunos-2.0.1",
            SnapshotOrigin::L2,
        )
        .await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        assert_eq!(result.unwrap(), 101);
        packages_mock.assert_async().await;
        tracking_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_resolve_tracking_id_from_repo_branch_ctyunos_major_prefix() {
        let (mut server, client) = setup_test_server().await;

        let packages_mock = server
            .mock("GET", "/api/packages")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!([
                    {
                        "id": 12,
                        "name": "gcc",
                        "level": 1,
                        "sync_interval_hours": 24,
                        "l0_repo_url": "https://github.com/test/test",
                        "description": "Test package",
                        "created_at": "2024-01-01T00:00:00Z",
                        "updated_at": "2024-01-01T00:00:00Z"
                    }
                ])
                .to_string(),
            )
            .create_async()
            .await;

        let tracking_mock = server
            .mock("GET", "/api/tracking?page=1&page_size=100&package_id=12")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "code": 200,
                    "message": "success",
                    "data": {
                        "items": [
                            {
                                "id": 102,
                                "package_id": 12,
                                "distro_id": 1,
                                "l1_repo_owner": "test-owner",
                                "l1_repo_name": "test-repo",
                                "l1_branch": "main",
                                "l2_branch": "25.07",
                                "l2_repo_path": "/path/to/repo",
                                "tracking_status": "active",
                                "last_sync_time": null,
                                "last_l1_commit_sha": null,
                                "last_l2_commit_sha": null,
                                "created_at": "2024-01-01T00:00:00Z",
                                "updated_at": "2024-01-01T00:00:00Z"
                            }
                        ],
                        "total": 1
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = resolve_tracking_id_from_repo_branch(
            &client,
            "gcc",
            "ctyunos-4-25.07",
            SnapshotOrigin::L2,
        )
        .await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        assert_eq!(result.unwrap(), 102);
        packages_mock.assert_async().await;
        tracking_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_resolve_tracking_id_tracking_empty() {
        let (mut server, client) = setup_test_server().await;

        let packages_mock = server
            .mock("GET", "/api/packages")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!([
                    {
                        "id": 10,
                        "name": "test-package",
                        "level": 1,
                        "sync_interval_hours": 24,
                        "l0_repo_url": "https://github.com/test/test",
                        "description": "Test package",
                        "created_at": "2024-01-01T00:00:00Z",
                        "updated_at": "2024-01-01T00:00:00Z"
                    }
                ])
                .to_string(),
            )
            .create_async()
            .await;

        let tracking_mock = server
            .mock("GET", "/api/tracking?page=1&page_size=100&package_id=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "code": 200,
                    "message": "success",
                    "data": {
                        "items": [],
                        "total": 0
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let err = resolve_tracking_id_from_repo_branch(
            &client,
            "test-package",
            "main",
            SnapshotOrigin::L1,
        )
        .await
        .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("没有关联的 tracking 配置"));
        packages_mock.assert_async().await;
        tracking_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_resolve_tracking_id_empty_response() {
        let (mut server, client) = setup_test_server().await;

        let packages_mock = server
            .mock("GET", "/api/packages")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!([
                    {
                        "id": 10,
                        "name": "test-package",
                        "level": 1,
                        "sync_interval_hours": 24,
                        "l0_repo_url": "https://github.com/test/test",
                        "description": "Test package",
                        "created_at": "2024-01-01T00:00:00Z",
                        "updated_at": "2024-01-01T00:00:00Z"
                    }
                ])
                .to_string(),
            )
            .create_async()
            .await;

        let tracking_mock = server
            .mock("GET", "/api/tracking?page=1&page_size=100&package_id=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "code": 200,
                    "message": "success",
                    "data": null
                })
                .to_string(),
            )
            .create_async()
            .await;

        let err = resolve_tracking_id_from_repo_branch(
            &client,
            "test-package",
            "main",
            SnapshotOrigin::L1,
        )
        .await
        .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("没有关联的 tracking 配置"));
        packages_mock.assert_async().await;
        tracking_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_resolve_tracking_id_package_not_found() {
        let (mut server, client) = setup_test_server().await;

        let packages_mock = server
            .mock("GET", "/api/packages")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::json!([]).to_string())
            .create_async()
            .await;

        let result = resolve_tracking_id_from_repo_branch(
            &client,
            "nonexistent",
            "main",
            SnapshotOrigin::L1,
        )
        .await;
        assert!(result.is_err());
        packages_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_import_batch_files_partial_failure_bails() {
        let (mut server, client) = setup_test_server().await;

        let file1_json = serde_json::json!({
            "repo": "test-package",
            "tracking_id": 100,
            "generated_at": "2024-01-01T00:00:00Z",
            "origin": "L1",
            "files": [
                {"path": "a.patch", "sha256": "x", "size": 1, "is_binary": false}
            ],
            "spec": null,
            "commits": [],
            "issues": []
        })
        .to_string();

        let mut file1 = NamedTempFile::new().unwrap();
        file1.write_all(file1_json.as_bytes()).unwrap();
        file1.flush().unwrap();

        let file2_json = serde_json::json!({
            "repo": "test-package",
            "branch": "missing-branch",
            "level": "l1",
            "commits": []
        })
        .to_string();

        let mut file2 = NamedTempFile::new().unwrap();
        file2.write_all(file2_json.as_bytes()).unwrap();
        file2.flush().unwrap();

        let packages_mock = server
            .mock("GET", "/api/packages")
            .expect(1)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!([
                    {
                        "id": 10,
                        "name": "test-package",
                        "level": 1,
                        "sync_interval_hours": 24,
                        "l0_repo_url": "https://github.com/test/test",
                        "description": "Test package",
                        "created_at": "2024-01-01T00:00:00Z",
                        "updated_at": "2024-01-01T00:00:00Z"
                    }
                ])
                .to_string(),
            )
            .create_async()
            .await;

        let tracking_mock = server
            .mock("GET", "/api/tracking?page=1&page_size=100&package_id=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "code": 200,
                    "message": "success",
                    "data": {
                        "items": [
                            {
                                "id": 100,
                                "package_id": 10,
                                "distro_id": 1,
                                "l1_repo_owner": "test-owner",
                                "l1_repo_name": "test-repo",
                                "l1_branch": "main",
                                "l2_branch": "openeuler",
                                "l2_repo_path": "/path/to/repo",
                                "tracking_status": "active",
                                "last_sync_time": null,
                                "last_l1_commit_sha": null,
                                "last_l2_commit_sha": null,
                                "created_at": "2024-01-01T00:00:00Z",
                                "updated_at": "2024-01-01T00:00:00Z"
                            }
                        ],
                        "total": 1
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let import_mock = server
            .mock("POST", "/api/metadata/l1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "code": 200,
                    "message": "success",
                    "data": {
                        "snapshot_id": "snap-batch",
                        "tracking_id": 100,
                        "file_count": 1,
                        "imported_at": "2024-01-01T00:00:00Z"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = import_batch_files(
            &client,
            vec![file1.path().to_path_buf(), file2.path().to_path_buf()],
            None,
        )
        .await;
        assert!(result.is_err());
        packages_mock.assert_async().await;
        tracking_mock.assert_async().await;
        import_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_execute_metadata_action() {
        let (mut server, client) = setup_test_server().await;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write_all(create_test_snapshot_json().as_bytes())
            .unwrap();
        temp_file.flush().unwrap();

        let mock = server
            .mock("POST", "/api/metadata/l1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "code": 200,
                    "message": "success",
                    "data": {
                        "snapshot_id": "snap-exec-test",
                        "tracking_id": 1,
                        "file_count": 5,
                        "imported_at": "2024-01-01T00:00:00Z"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let action = ImportAction::Metadata {
            file: temp_file.path().to_str().unwrap().to_string(),
            tracking_id: Some(1),
        };
        let result = execute(&client, action).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }
}
