/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. ctscat is licensed under Mulan PSL v2. You can use this software
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
            import_single_file(api_client, &path, tracking_id).await
        }
        ImportAction::Batch {
            files,
            tracking_id: _tracking_id,
        } => {
            let paths: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
            import_batch_files(api_client, paths).await
        }
    }
}

/// 从 JSON 内容中提取 repo 字段（package name）
fn extract_repo_from_json(content: &str) -> Result<String> {
    let root: Value =
        serde_json::from_str(content).map_err(|e| anyhow!("解析 JSON 失败: {}", e))?;

    // 尝试从通用采集格式提取 repo 字段
    if let Some(repo) = root.get("repo").and_then(|v| v.as_str()) {
        return Ok(repo.to_string());
    }

    // 如果是标准 RepositorySnapshot 格式，没有 repo 字段，需要提供 tracking_id
    Err(anyhow!(
        "JSON 文件中未找到 'repo' 字段，请使用 --tracking-id 参数明确指定"
    ))
}

/// 根据 package name 查询对应的 tracking_id
async fn resolve_tracking_id_from_package(
    api_client: &ApiClient,
    package_name: &str,
) -> Result<i32> {
    // 1. 查询 package 列表，找到匹配的 package_id
    let packages: Vec<PackageDto> = api_client
        .get("/packages")
        .await
        .map_err(|e| anyhow!("查询 package 列表失败: {}", e))?;

    let package = packages
        .iter()
        .find(|p| p.name == package_name)
        .ok_or_else(|| anyhow!("未找到名称为 '{}' 的 package", package_name))?;

    let package_id = package.id;

    // 2. 查询该 package 的 tracking 配置
    let query = format!("?page=1&page_size=100&package_id={}", package_id);
    let response: ApiResponse<ListResponse<TrackingDto>> = api_client
        .get(&format!("/tracking{}", query))
        .await
        .map_err(|e| anyhow!("查询 tracking 配置失败: {}", e))?;

    let trackings = response.data.ok_or_else(|| anyhow!("空响应"))?.items;

    if trackings.is_empty() {
        return Err(anyhow!(
            "package '{}' (ID: {}) 没有关联的 tracking 配置，请先创建",
            package_name,
            package_id
        ));
    }

    // 3. 优先选择状态为 active 的 tracking
    let active_tracking = trackings
        .iter()
        .find(|t| t.tracking_status == "active")
        .or_else(|| trackings.first());

    if let Some(tracking) = active_tracking {
        if trackings.len() > 1 {
            println!(
                "  {} package '{}' 有 {} 个 tracking 配置，使用 tracking_id: {} (状态: {})",
                "ℹ".cyan(),
                package_name,
                trackings.len(),
                tracking.id,
                tracking.tracking_status
            );
        }
        Ok(tracking.id)
    } else {
        Err(anyhow!("未找到可用的 tracking 配置"))
    }
}

/// 导入单个文件
async fn import_single_file(
    api_client: &ApiClient,
    file: &PathBuf,
    tracking_id: i32,
) -> Result<()> {
    println!("正在导入文件: {}", file.display().to_string().cyan());

    // 检查文件是否存在
    if !file.exists() {
        println!("{} 文件不存在: {}", "✗".red().bold(), file.display());
        anyhow::bail!("文件不存在");
    }

    // 读取并解析/转换快照文件
    let content = fs::read_to_string(file)?;
    let snapshot = parse_snapshot_or_convert(&content, tracking_id)?;

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
        tracking_id,
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
async fn import_batch_files(api_client: &ApiClient, files: Vec<PathBuf>) -> Result<()> {
    println!("开始批量导入 {} 个文件", files.len());
    println!();

    let mut success_count = 0;
    let mut failed_count = 0;
    let mut total_files = 0;

    for (i, file) in files.iter().enumerate() {
        println!("[{}/{}] 导入文件: {}", i + 1, files.len(), file.display());

        let content = fs::read_to_string(file)?;

        let package_name = extract_repo_from_json(&content)?;
        // 从 JSON 文件解析 tracking_id
        match resolve_tracking_id_from_package(api_client, &package_name).await {
            Ok(tracking_id) => {
                println!("  解析到 tracking_id: {}", tracking_id);
                match import_single_file(api_client, file, tracking_id).await {
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
            Err(e) => {
                println!("  无法解析 tracking_id: {}", e);
                failed_count += 1;
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
    fn test_extract_repo_from_json() {
        let json = r#"{"repo": "test-package", "commits": []}"#;
        let result = extract_repo_from_json(json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-package");
    }

    #[test]
    fn test_extract_repo_from_json_missing() {
        let json = r#"{"commits": []}"#;
        let result = extract_repo_from_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_repo_from_json_invalid_json() {
        let result = extract_repo_from_json("not-json");
        assert!(result.is_err());
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
