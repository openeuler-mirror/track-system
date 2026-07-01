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

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::collectors::traits::GitClient;
use crate::utils::spec::SpecParser;
use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use chrono::Utc;
use regex::Regex;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, JsonValue, QueryFilter,
    QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{error, info};
use walkdir::WalkDir;

use crate::{
    entities::{
        l2_snapshots,
        prelude::{
            Issues, L1CommitRecords, L2CommitRecords, L2Snapshots, Tracking as TrackingEntity,
        },
    },
    snapshot::types::{
        ChangeStats, CommitEntry, FileEntry, IssueEntry, RepositorySnapshot, SnapshotOrigin,
        SpecEntry,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotSummary {
    pub tracking_id: i32,
    pub checksum: String,
    pub file_count: usize,
    pub spec_version: Option<String>,
    pub commit_count: usize,
    pub issue_count: usize,
}

pub async fn export_l2_snapshot<P: AsRef<Path>, Q: AsRef<Path>>(
    db: &DatabaseConnection,
    tracking_id: i32,
    repo_path: P,
    output_path: Q,
) -> Result<SnapshotSummary> {
    let tracking = TrackingEntity::find_by_id(tracking_id)
        .one(db)
        .await?
        .context("tracking configuration not found")?;

    let repo_path_buf = repo_path.as_ref().to_path_buf();
    let snapshot =
        build_repository_snapshot(db, &tracking, SnapshotOrigin::L2, Some(&repo_path_buf)).await?;
    persist_snapshot(db, &snapshot, output_path.as_ref()).await
}

pub async fn export_l1_snapshot<Q: AsRef<Path>>(
    db: &DatabaseConnection,
    tracking_id: i32,
    repo_path: Option<PathBuf>,
    output_path: Q,
) -> Result<SnapshotSummary> {
    let tracking = TrackingEntity::find_by_id(tracking_id)
        .one(db)
        .await?
        .context("tracking configuration not found")?;

    let repo_path_ref = repo_path.as_deref();
    let snapshot =
        build_repository_snapshot(db, &tracking, SnapshotOrigin::L1, repo_path_ref).await?;
    persist_snapshot(db, &snapshot, output_path.as_ref()).await
}

pub async fn import_snapshot<P: AsRef<Path>>(
    db: &DatabaseConnection,
    tracking_id: i32,
    input_path: P,
) -> Result<SnapshotSummary> {
    let tracking = TrackingEntity::find_by_id(tracking_id)
        .one(db)
        .await?
        .context("tracking configuration not found")?;

    let json = fs::read_to_string(input_path.as_ref())?;
    let snapshot: RepositorySnapshot = serde_json::from_str(&json)?;

    if snapshot.tracking_id != tracking.id {
        bail!(
            "snapshot tracking_id {} does not match target {}",
            snapshot.tracking_id,
            tracking.id
        );
    }

    persist_snapshot(db, &snapshot, input_path.as_ref()).await
}

pub async fn latest_snapshot(
    db: &DatabaseConnection,
    tracking_id: i32,
) -> Result<Option<RepositorySnapshot>> {
    let record = L2Snapshots::find()
        .filter(l2_snapshots::Column::TrackingId.eq(tracking_id))
        .order_by_desc(l2_snapshots::Column::CreatedAt)
        .one(db)
        .await?;

    if let Some(model) = record {
        let snapshot: RepositorySnapshot = serde_json::from_value(model.payload)?;
        Ok(Some(snapshot))
    } else {
        Ok(None)
    }
}

async fn build_repository_snapshot(
    db: &DatabaseConnection,
    tracking: &crate::entities::tracking::Model,
    origin: SnapshotOrigin,
    repo_path: Option<&Path>,
) -> Result<RepositorySnapshot> {
    if let Some(path) = repo_path {
        if !path.exists() {
            bail!("repo path {} does not exist", path.display());
        }
    }

    // 特殊处理：L2 无 repo_path 时，尝试从 l2_snapshots 表加载
    if matches!(origin, SnapshotOrigin::L2) && repo_path.is_none() {
        if let Some(snapshot) = load_l2_snapshot_from_db(db, tracking.id).await? {
            return Ok(snapshot);
        }
        // 如果没有找到快照数据，回退到从数据库读取 commits
        info!(
            tracking_id = tracking.id,
            "未找到 L2 快照数据，回退到从数据库读取 commits"
        );
    }

    let mut snapshot = RepositorySnapshot::new(tracking.id, origin.clone());
    snapshot.spec = collect_spec(origin.clone(), tracking, repo_path).await?;
    snapshot.files = collect_files(origin.clone(), snapshot.spec.as_ref(), repo_path)?;
    snapshot.commits = match (origin.clone(), repo_path) {
        (SnapshotOrigin::L2, Some(path)) => {
            // 从 repo 收集 L2 commits 并持久化到数据库
            let result = collect_commits_from_repo(path)?;
            info!(
                tracking_id = tracking.id,
                commit_count = result.commits.len(),
                spec_version = ?result.spec_version,
                spec_release = ?result.spec_release,
                "从仓库收集到 commits 和 spec 信息"
            );
            persist_l2_commits(
                db,
                tracking.id,
                &result.commits,
                result.spec_version.as_deref(),
                result.spec_release.as_deref(),
            )
            .await?;
            // 再从数据库读取以保持一致性
            collect_l2_commits(db, tracking.id).await?
        }
        (SnapshotOrigin::L2, None) => {
            // 无 repo 路径时，从数据库读取 L2 commits
            collect_l2_commits(db, tracking.id).await?
        }
        _ => collect_commits(db, tracking.id).await?,
    };
    snapshot.issues = collect_issues(db, tracking.id).await?;

    Ok(snapshot)
}
fn collect_files(
    _origin: SnapshotOrigin,
    spec_entry: Option<&SpecEntry>,
    repo_path: Option<&Path>,
) -> Result<Vec<FileEntry>> {
    let mut entries: Vec<FileEntry> = Vec::new();

    // 1) 如果提供了本地仓库路径，按原逻辑遍历文件系统
    if let Some(root) = repo_path {
        for entry in WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| e.file_name() != ".git")
        {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let rel_path = entry
                .path()
                .strip_prefix(root)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('\\', "/");

            let content = fs::read(entry.path())?;
            let sha = sha256_hex(&content);
            let size = content.len() as u64;
            let is_binary = std::str::from_utf8(&content).is_err();

            entries.push(FileEntry {
                path: rel_path,
                size,
                sha256: sha,
                is_binary,
            });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        return Ok(entries);
    }

    // 2) 当没有本地仓库可遍历、且存在 spec 时，基于 spec 提取补丁与源文件
    if let Some(spec) = spec_entry {
        // 解码 spec 内容
        let decoded = BASE64_STANDARD
            .decode(spec.content_base64.replace('\n', "").as_bytes())
            .map_err(|e| anyhow::anyhow!("Base64 解码 spec 内容失败: {}", e))?;
        let spec_text = String::from_utf8(decoded)
            .map_err(|e| anyhow::anyhow!("spec 内容不是有效的 UTF-8: {}", e))?;

        // 解析 spec，提取 patches 与 sources
        let parsed = SpecParser::parse(&spec_text)?;

        // 生成 FileEntry：补丁文件
        for p in parsed.patches.iter() {
            // 使用规范化的相对路径（仅作为占位），确保后续提取识别 .patch/.diff
            let path = p.trim().to_string();
            if path.is_empty() {
                continue;
            }
            // 内容哈希无法获取，采用路径字符串的哈希作为占位，避免空值
            let sha = sha256_hex(path.as_bytes());
            entries.push(FileEntry {
                path,
                size: 0,
                sha256: sha,
                is_binary: false,
            });
        }

        // 生成 FileEntry：源文件（例如 tarball 等）
        for s in parsed.sources.iter() {
            let path = s.trim().to_string();
            if path.is_empty() {
                continue;
            }
            let sha = sha256_hex(path.as_bytes());
            // 根据扩展名粗略判断二进制性质
            let is_binary = path.ends_with(".tar")
                || path.ends_with(".tar.gz")
                || path.ends_with(".tar.bz2")
                || path.ends_with(".xz")
                || path.ends_with(".zip")
                || path.ends_with(".tgz");
            entries.push(FileEntry {
                path,
                size: 0,
                sha256: sha,
                is_binary,
            });
        }

        // 去重与排序
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        entries.dedup_by(|a, b| a.path == b.path);
        return Ok(entries);
    }

    // 3) 无仓库、无 spec 的情况，返回空列表
    Ok(entries)
}

async fn collect_spec(
    origin: SnapshotOrigin,
    tracking: &crate::entities::tracking::Model,
    repo_path: Option<&Path>,
) -> Result<Option<SpecEntry>> {
    // 1) 优先从本地仓库读取（与原实现保持向后兼容）
    if let Some(root) = repo_path {
        info!(
            tracking_id = tracking.id,
            repo_path = %root.display(),
            "开始收集本地仓库 spec 文件"
        );
        for entry in WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| e.file_name() != ".git")
        {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            if entry
                .path()
                .extension()
                .map(|ext| ext == "spec")
                .unwrap_or(false)
            {
                let content = fs::read(entry.path())?;
                let sha = sha256_hex(&content);
                let version = std::str::from_utf8(&content)
                    .ok()
                    .and_then(extract_spec_version);
                let release = std::str::from_utf8(&content)
                    .ok()
                    .and_then(extract_spec_release);
                return Ok(Some(SpecEntry {
                    path: entry
                        .path()
                        .strip_prefix(root)
                        .unwrap_or(entry.path())
                        .to_string_lossy()
                        .replace('\\', "/"),
                    sha256: sha,
                    version,
                    release,
                    content_base64: BASE64_STANDARD.encode(content),
                }));
            }
        }
        return Ok(None);
    }

    // 2) 对于 L1 仓库，使用 Gitee API 获取 spec 内容
    if origin == SnapshotOrigin::L1 {
        info!(
            tracking_id = tracking.id,
            l1_repo_owner = %tracking.l1_repo_owner,
            l1_repo_name = %tracking.l1_repo_name,
            l1_branch = %tracking.l1_branch,
            "开始从 Gitee 获取 L1 spec 文件"
        );
        let owner = tracking.l1_repo_owner.as_str();
        let repo = tracking.l1_repo_name.as_str();
        let branch = tracking.l1_branch.as_str();
        let spec_path = crate::component::normalize_spec_path(repo, None);

        let token = std::env::var("GITEE_ACCESS_TOKEN")
            .or_else(|_| std::env::var("GITEE_TOKEN"))
            .unwrap_or_default();
        let client = match crate::collectors::GiteeClient::new(token) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(owner = %owner, repo = %repo, branch = %branch, spec_path = %spec_path, error = %e, "创建 Gitee 客户端失败，无法获取 L1 spec");
                return Ok(None);
            }
        };

        match client
            .get_file_content(owner, repo, &spec_path, branch)
            .await
        {
            Ok(file) => {
                // Gitee 返回的 content 为 Base64，需要解码以提取版本与计算 SHA256
                let normalized = file.content.replace('\n', "");
                let bytes = match BASE64_STANDARD.decode(normalized.as_bytes()) {
                    Ok(b) => b,
                    Err(err) => {
                        tracing::error!(owner = %owner, repo = %repo, branch = %branch, spec_path = %spec_path, error = %err, "Base64 解码 spec 内容失败");
                        return Ok(None);
                    }
                };
                let sha = sha256_hex(&bytes);
                let version = String::from_utf8(bytes.clone())
                    .ok()
                    .and_then(|s| extract_spec_version(&s));
                let release = String::from_utf8(bytes.clone())
                    .ok()
                    .and_then(|s| extract_spec_release(&s));

                return Ok(Some(SpecEntry {
                    path: file.path,
                    sha256: sha,
                    version,
                    release,
                    content_base64: normalized,
                }));
            }
            Err(err) => {
                tracing::error!(owner = %owner, repo = %repo, branch = %branch, spec_path = %spec_path, error = %err, "获取 Gitee spec 文件失败");
                return Ok(None);
            }
        }
    }

    // 3) 其他情况（无本地仓库路径且非 L1），不采集 spec
    Ok(None)
}

async fn collect_commits(db: &DatabaseConnection, tracking_id: i32) -> Result<Vec<CommitEntry>> {
    let models = L1CommitRecords::find()
        .filter(crate::entities::l1_commit_records::Column::TrackingId.eq(tracking_id))
        .order_by_desc(crate::entities::l1_commit_records::Column::CommittedAt)
        .all(db)
        .await?;

    let entries = models
        .into_iter()
        .map(|model| {
            let cve_list: Vec<String> = model
                .cve_list
                .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
                .unwrap_or_default();

            CommitEntry {
                sha: model.commit_sha,
                title: model
                    .commit_message
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string(),
                message: model.commit_message,
                author: model.author_name,
                authored_at: model.committed_at,
                url: Some(model.api_url),
                stats: crate::snapshot::types::ChangeStats {
                    additions: model.additions,
                    deletions: model.deletions,
                    files_changed: model.files_changed_count,
                },
                primary_change_type: model.primary_change_type,
                cve_list,
            }
        })
        .collect();

    Ok(entries)
}

async fn collect_issues(db: &DatabaseConnection, tracking_id: i32) -> Result<Vec<IssueEntry>> {
    let models = Issues::find()
        .filter(crate::entities::issues::Column::TrackingId.eq(tracking_id))
        .order_by_desc(crate::entities::issues::Column::UpdatedAt)
        .all(db)
        .await?;

    let entries = models
        .into_iter()
        .map(|model| {
            let labels = model
                .labels
                .and_then(|value| {
                    serde_json::from_value::<Vec<String>>(value.clone())
                        .or_else(|_| serde_json::from_value::<String>(value).map(|s| vec![s]))
                        .ok()
                })
                .unwrap_or_default();

            IssueEntry {
                number: model.issue_number,
                title: model.title,
                state: model.state,
                author: model.author,
                labels,
                updated_at: model.updated_at,
            }
        })
        .collect();

    Ok(entries)
}

async fn persist_snapshot<P: AsRef<Path>>(
    db: &DatabaseConnection,
    snapshot: &RepositorySnapshot,
    output_path: P,
) -> Result<SnapshotSummary> {
    let json = serde_json::to_string_pretty(snapshot)?;
    fs::write(output_path.as_ref(), &json)?;

    let checksum = sha256_hex(json.as_bytes());
    let payload_value: JsonValue = serde_json::from_str(&json)?;

    let model = l2_snapshots::ActiveModel {
        tracking_id: Set(snapshot.tracking_id),
        snapshot_type: Set(match snapshot.origin {
            SnapshotOrigin::L1 => "l1".to_string(),
            SnapshotOrigin::L2 => "l2".to_string(),
            SnapshotOrigin::Unknown => "unknown".to_string(),
        }),
        checksum: Set(checksum.clone()),
        payload: Set(payload_value),
        created_at: Set(Utc::now()),
        ..Default::default()
    }
    .insert(db)
    .await?;

    info!(
        snapshot_id = model.id,
        tracking_id = snapshot.tracking_id,
        checksum = checksum,
        file_count = snapshot.files.len(),
        origin = ?snapshot.origin,
        "snapshot stored"
    );

    Ok(SnapshotSummary {
        tracking_id: snapshot.tracking_id,
        checksum,
        file_count: snapshot.files.len(),
        spec_version: snapshot.spec.as_ref().and_then(|s| s.version.clone()),
        commit_count: snapshot.commits.len(),
        issue_count: snapshot.issues.len(),
    })
}

fn extract_spec_version(content: &str) -> Option<String> {
    let re = Regex::new(r"(?m)^\s*Version\s*:\s*([\w\.\-]+)").ok()?;
    re.captures(content)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_spec_release(content: &str) -> Option<String> {
    let re = Regex::new(r"(?m)^\s*Release\s*:\s*([^\r\n]+)").ok()?;
    re.captures(content)
        .and_then(|caps| caps.get(1))
        .map(|m| {
            let raw = m.as_str().trim();
            // 去掉常见的可选宏与尾随右括号
            let cleaned = raw
                .replace("%{?dist}", "")
                .replace("%{?scl:", "")
                .replace("%{!?scl:", "")
                .replace("%{?scl_prefix}", "")
                .replace('}', "")
                .trim()
                .to_string();
            cleaned
        })
        .filter(|s| !s.is_empty())
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    format!("{:x}", digest)
}

#[derive(Debug, Clone)]
pub struct RepoCollectionResult {
    pub commits: Vec<CommitEntry>,
    pub spec_version: Option<String>,
    pub spec_release: Option<String>,
}

fn collect_commits_from_repo(repo_path: &Path) -> Result<RepoCollectionResult> {
    // 1. 解析 spec 文件获取 version 和 release
    let (spec_version, spec_release) = parse_spec_from_repo(repo_path)?;

    // 2. 通过 git 命令采集提交信息：SHA、作者、时间、标题，并统计更改数量
    let repo = repo_path.to_string_lossy().to_string();
    let output = Command::new("git")
        .args([
            "-C",
            &repo,
            "log",
            "--date=iso-strict",
            "--pretty=format:%H%x1f%an%x1f%ae%x1f%ad%x1f%s",
            "--numstat",
        ])
        .output()
        .with_context(|| format!("执行 git log 失败: {}", repo))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(
            repo = %repo,
            code = ?output.status.code(),
            stderr = %stderr,
            "git log 执行失败"
        );
        bail!("git log 执行失败: {}", stderr.trim());
    }

    let delim = '\u{001f}'; // 自定义分隔符 0x1F
    let mut commits: Vec<CommitEntry> = Vec::new();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut current: Option<(CommitEntry, i32, i32, i32)> = None; // (entry, additions, deletions, files_changed)

    let cve_re = Regex::new(r"CVE-\d{4}-\d{4,7}").unwrap();

    for line in stdout.lines() {
        if line.contains(delim) {
            // 遇到新的提交头，先收尾上一个提交
            if let Some((mut entry, adds, dels, files)) = current.take() {
                entry.stats = ChangeStats {
                    additions: adds,
                    deletions: dels,
                    files_changed: files,
                };
                commits.push(entry);
            }

            // 解析当前提交头
            let parts: Vec<&str> = line.split(delim).collect();
            if parts.len() < 5 {
                // 头部格式不符合预期，跳过
                continue;
            }

            let sha = parts[0].to_string();
            let author = parts[1].to_string();
            let _email = parts[2];
            let authored_at_str = parts[3];
            let title = parts[4].to_string();
            let authored_at = chrono::DateTime::parse_from_rfc3339(authored_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            // 从标题提取 CVE
            let mut cve_list: Vec<String> = Vec::new();
            for m in cve_re.find_iter(&title) {
                cve_list.push(m.as_str().to_string());
            }

            let entry = CommitEntry {
                sha,
                title: title.clone(),
                message: title, // 暂以标题作为消息摘要
                author,
                authored_at,
                url: None,
                stats: ChangeStats::default(),
                primary_change_type: None,
                cve_list,
            };

            current = Some((entry, 0, 0, 0));
        } else {
            // 解析 numstat 行：<additions> <deletions> <path>
            if let Some((entry, mut adds, mut dels, mut files)) = current.take() {
                let cols: Vec<&str> = line.split_whitespace().collect();
                if cols.len() >= 3 {
                    let add = cols[0].parse::<i32>().ok();
                    let del = cols[1].parse::<i32>().ok();
                    if let (Some(a), Some(d)) = (add, del) {
                        adds += a;
                        dels += d;
                        files += 1;
                    }
                }
                current = Some((entry, adds, dels, files));
            }
        }
    }

    // 收尾最后一个提交
    if let Some((mut entry, adds, dels, files)) = current.take() {
        entry.stats = ChangeStats {
            additions: adds,
            deletions: dels,
            files_changed: files,
        };
        commits.push(entry);
    }

    // 以时间降序排列
    commits.sort_by(|a, b| b.authored_at.cmp(&a.authored_at));

    Ok(RepoCollectionResult {
        commits,
        spec_version,
        spec_release,
    })
}

/// 从仓库路径中解析 spec 文件，提取 version 和 release
fn parse_spec_from_repo(repo_path: &Path) -> Result<(Option<String>, Option<String>)> {
    // 遍历仓库目录查找 .spec 文件
    for entry in WalkDir::new(repo_path)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        // 检查是否为 .spec 文件
        if entry
            .path()
            .extension()
            .map(|ext| ext == "spec")
            .unwrap_or(false)
        {
            // 读取 spec 文件内容
            let content = fs::read(entry.path())?;
            let spec_text = String::from_utf8_lossy(&content);

            // 解析 version 和 release
            let version = extract_spec_version(&spec_text);
            let release = extract_spec_release(&spec_text);

            info!(
                spec_path = %entry.path().display(),
                version = ?version,
                release = ?release,
                "解析到 spec 文件"
            );

            return Ok((version, release));
        }
    }

    // 未找到 spec 文件
    Ok((None, None))
}

// 从数据库收集 L2 commits
/// 从 l2_snapshots 表加载快照数据并解析
async fn load_l2_snapshot_from_db(
    db: &DatabaseConnection,
    tracking_id: i32,
) -> Result<Option<RepositorySnapshot>> {
    use crate::collectors::traits::CollectResult;

    // 查询最新的 L2 快照
    let snapshot_record = L2Snapshots::find()
        .filter(l2_snapshots::Column::TrackingId.eq(tracking_id))
        .order_by_desc(l2_snapshots::Column::CreatedAt)
        .one(db)
        .await?;

    let snapshot_record = match snapshot_record {
        Some(record) => record,
        None => {
            info!(tracking_id = tracking_id, "未找到 L2 快照数据");
            return Ok(None);
        }
    };

    info!(
        tracking_id = tracking_id,
        snapshot_id = snapshot_record.id,
        "从数据库加载 L2 快照数据"
    );

    // 解析 payload JSON
    let collect_result: CollectResult =
        serde_json::from_value(snapshot_record.payload).context("解析 L2 快照 payload 失败")?;

    // 转换 commits 并持久化到 l2_commit_records
    let commit_entries: Vec<CommitEntry> = collect_result
        .commits
        .iter()
        .map(|c| CommitEntry {
            sha: c.sha.clone(),
            title: c.title.clone(),
            message: c.message.clone(),
            author: c.author.clone(),
            authored_at: c.date,
            url: None, // CollectResult 中没有 URL 字段
            stats: ChangeStats {
                additions: 0,
                deletions: 0,
                files_changed: 0,
            },
            primary_change_type: None,
            cve_list: Vec::new(),
        })
        .collect();

    // 提取 spec 信息用于持久化
    let spec_version = collect_result.spec.as_ref().map(|s| s.version.as_str());
    let spec_release = collect_result.spec.as_ref().map(|s| s.release.as_str());

    // 持久化 commits 到数据库
    if !commit_entries.is_empty() {
        persist_l2_commits(db, tracking_id, &commit_entries, spec_version, spec_release).await?;
        info!(
            tracking_id = tracking_id,
            commit_count = commit_entries.len(),
            "已将 L2 快照中的 commits 持久化到数据库"
        );
    }

    // 转换 spec 信息
    let spec_entry = collect_result.spec.map(|s| SpecEntry {
        path: s.path,
        version: Some(s.version),
        release: Some(s.release),
        sha256: s.sha256,
        content_base64: s.content_base64,
    });

    // 转换 files 信息
    let file_entries: Vec<FileEntry> = collect_result
        .files
        .iter()
        .map(|f| FileEntry {
            path: f.path.clone(),
            size: f.size,
            sha256: f.sha256.clone(),
            is_binary: f.is_binary,
        })
        .collect();

    // 转换 issues 信息
    let issue_entries: Vec<IssueEntry> = collect_result
        .issues
        .iter()
        .map(|i| IssueEntry {
            number: i.number.to_string(),
            title: i.title.clone(),
            state: i.state.clone(),
            author: i.author.clone(),
            updated_at: i.updated_at,
            labels: i.labels.clone(),
        })
        .collect();

    // 构建 RepositorySnapshot
    let mut snapshot = RepositorySnapshot::new(tracking_id, SnapshotOrigin::L2);
    snapshot.spec = spec_entry;
    snapshot.files = file_entries;
    snapshot.commits = commit_entries;
    snapshot.issues = issue_entries;

    Ok(Some(snapshot))
}
async fn collect_l2_commits(db: &DatabaseConnection, tracking_id: i32) -> Result<Vec<CommitEntry>> {
    let models = L2CommitRecords::find()
        .filter(crate::entities::l2_commit_records::Column::TrackingId.eq(tracking_id))
        .order_by_desc(crate::entities::l2_commit_records::Column::CommittedAt)
        .all(db)
        .await?;

    let entries = models
        .into_iter()
        .map(|model| {
            let cve_list: Vec<String> = model
                .cve_list
                .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
                .unwrap_or_default();

            CommitEntry {
                sha: model.commit_sha,
                title: model
                    .commit_message
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string(),
                message: model.commit_message,
                author: model.author_name,
                authored_at: model.committed_at,
                url: Some(model.api_url),
                stats: crate::snapshot::types::ChangeStats {
                    additions: model.additions,
                    deletions: model.deletions,
                    files_changed: model.files_changed_count,
                },
                primary_change_type: model.primary_change_type,
                cve_list,
            }
        })
        .collect();

    Ok(entries)
}

// 持久化 L2 commits 到数据库
async fn persist_l2_commits(
    db: &DatabaseConnection,
    tracking_id: i32,
    commits: &[CommitEntry],
    spec_version: Option<&str>,
    spec_release: Option<&str>,
) -> Result<()> {
    use crate::entities::l2_commit_records;
    use sea_orm::sea_query::OnConflict;

    for commit in commits {
        let cve_json = if commit.cve_list.is_empty() {
            None
        } else {
            Some(serde_json::to_value(&commit.cve_list)?)
        };

        let model = l2_commit_records::ActiveModel {
            tracking_id: Set(tracking_id),
            commit_sha: Set(commit.sha.clone()),
            commit_message: Set(commit.message.clone()),
            author_name: Set(commit.author.clone()),
            author_email: Set(String::new()), // 从 repo 收集时没有 email
            committed_at: Set(commit.authored_at),
            change_type: Set(None),
            primary_change_type: Set(commit.primary_change_type.clone()),
            cve_list: Set(cve_json),
            spec_changed: Set(false),
            patch_stats: Set(None),
            classification_status: Set("pending".to_string()),
            classification_notes: Set(None),
            sync_status: Set("not_synced".to_string()),
            synced_to_l2_commit: Set(None),
            synced_at: Set(None),
            api_url: Set(commit.url.clone().unwrap_or_default()),
            fetched_at: Set(Utc::now()),
            files_changed_count: Set(commit.stats.files_changed),
            additions: Set(commit.stats.additions),
            deletions: Set(commit.stats.deletions),
            spec_version: Set(spec_version.map(|s| s.to_string())),
            spec_release: Set(spec_release.map(|s| s.to_string())),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        };

        // 使用 upsert 避免重复插入
        let _ = L2CommitRecords::insert(model)
            .on_conflict(
                OnConflict::columns([
                    l2_commit_records::Column::TrackingId,
                    l2_commit_records::Column::CommitSha,
                ])
                .update_columns([
                    l2_commit_records::Column::CommitMessage,
                    l2_commit_records::Column::AuthorName,
                    l2_commit_records::Column::CommittedAt,
                    l2_commit_records::Column::FilesChangedCount,
                    l2_commit_records::Column::Additions,
                    l2_commit_records::Column::Deletions,
                    l2_commit_records::Column::UpdatedAt,
                ])
                .to_owned(),
            )
            .exec(db)
            .await?;
    }

    info!(
        tracking_id = tracking_id,
        commit_count = commits.len(),
        "L2 commits 已持久化到数据库"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::l2_snapshots;
    use crate::entities::{issues, l1_commit_records, tracking};
    use crate::snapshot::types::SnapshotOrigin;
    use chrono::Utc;
    use sea_orm::{DatabaseBackend, MockDatabase};
    use serde_json::json;

    #[test]
    fn test_extract_spec_version_basic() {
        let content = "Name: a\nVersion: 1.2.3\nRelease: 1\n";
        assert_eq!(extract_spec_version(content), Some("1.2.3".to_string()));
    }

    #[test]
    fn test_extract_spec_version_with_spaces() {
        let content = "Version :   2.0.0-rc1\n";
        assert_eq!(extract_spec_version(content), Some("2.0.0-rc1".to_string()));
    }

    #[test]
    fn test_extract_spec_version_missing() {
        let content = "Name: a\nRelease: 1\n";
        assert_eq!(extract_spec_version(content), None);
    }

    #[test]
    fn test_extract_spec_release_strips_macros_and_braces() {
        let content = "Release: 9%{?dist}\n";
        assert_eq!(extract_spec_release(content), Some("9".to_string()));
    }

    #[test]
    fn test_extract_spec_release_strips_scl_macros() {
        let content = "Release: 1%{?scl:foo}%{!?scl:bar}%{?scl_prefix}\n";
        assert_eq!(extract_spec_release(content), Some("1foobar".to_string()));
    }

    #[test]
    fn test_sha256_hex_known_value() {
        let got = sha256_hex(b"hello world");
        assert_eq!(
            got,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_collect_files_from_spec_parses_patches_and_sources() {
        let spec_text = r#"
Name: test
Version: 1.0.0
Release: 1
Source0: source.tar.gz
Patch0: fix.patch
Patch1: fix.patch
"#;
        let spec = SpecEntry {
            path: "test.spec".to_string(),
            version: Some("1.0.0".to_string()),
            release: Some("1".to_string()),
            sha256: "s".to_string(),
            content_base64: BASE64_STANDARD.encode(spec_text),
        };

        let files = collect_files(SnapshotOrigin::L2, Some(&spec), None).unwrap();
        assert!(files.iter().any(|f| f.path.contains("fix.patch")));
        assert!(files.iter().any(|f| f.path.contains("source.tar.gz")));

        let patch_count = files.iter().filter(|f| f.path == "fix.patch").count();
        assert_eq!(patch_count, 1);
    }

    #[test]
    fn test_collect_files_spec_base64_error() {
        let spec = SpecEntry {
            path: "test.spec".to_string(),
            version: Some("1.0.0".to_string()),
            release: Some("1".to_string()),
            sha256: "s".to_string(),
            content_base64: "%%%not_base64%%%".to_string(),
        };

        let err = collect_files(SnapshotOrigin::L2, Some(&spec), None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("Base64 解码 spec 内容失败"));
    }

    #[test]
    fn test_collect_files_spec_non_utf8_error() {
        let spec = SpecEntry {
            path: "test.spec".to_string(),
            version: Some("1.0.0".to_string()),
            release: Some("1".to_string()),
            sha256: "s".to_string(),
            content_base64: BASE64_STANDARD.encode([0xffu8, 0xfeu8, 0xfdu8]),
        };

        let err = collect_files(SnapshotOrigin::L2, Some(&spec), None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("spec 内容不是有效的 UTF-8"));
    }

    #[tokio::test]
    async fn test_export_l1_snapshot_tracking_not_found() {
        use crate::entities::tracking;

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<tracking::Model, _, _>(vec![vec![]])
            .into_connection();

        let result = export_l1_snapshot(
            &db,
            1,
            None,
            PathBuf::from("/tmp/track-system-metadata-bridge-test.json"),
        )
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("tracking configuration not found"));
    }

    #[tokio::test]
    async fn test_import_snapshot_tracking_not_found() {
        use crate::entities::tracking;
        use std::io::Write;

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<tracking::Model, _, _>(vec![vec![]])
            .into_connection();

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("snapshot.json");
        let mut f = std::fs::File::create(&file_path).unwrap();
        write!(f, "{{\"tracking_id\": 1, \"origin\": \"L2\", \"files\": [], \"commits\": [], \"issues\": [], \"generated_at\": \"{}\"}}", Utc::now().to_rfc3339()).unwrap();

        let result = import_snapshot(&db, 1, &file_path).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("tracking configuration not found"));
    }

    #[tokio::test]
    async fn test_import_snapshot_tracking_id_mismatch() {
        use crate::entities::tracking;

        let tracking_model = tracking::Model {
            id: 2,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "o".to_string(),
            l1_repo_name: "r".to_string(),
            l2_branch: "b".to_string(),
            l2_repo_path: "/x".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: None,
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<tracking::Model, _, _>(vec![vec![tracking_model]])
            .into_connection();

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("snapshot.json");
        let snapshot = RepositorySnapshot::new(1, SnapshotOrigin::L2);
        std::fs::write(&file_path, serde_json::to_string(&snapshot).unwrap()).unwrap();

        let result = import_snapshot(&db, 2, &file_path).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not match"));
    }

    #[tokio::test]
    async fn test_latest_snapshot_none() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![]])
            .into_connection();

        let result = latest_snapshot(&db, 1).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_latest_snapshot_some() {
        let payload = json!({
            "tracking_id": 1,
            "origin": "L2",
            "spec": null,
            "files": [],
            "commits": [],
            "issues": [],
            "generated_at": Utc::now().to_rfc3339(),
        });
        let model = l2_snapshots::Model {
            id: 1,
            tracking_id: 1,
            snapshot_type: "l2".to_string(),
            checksum: "c".to_string(),
            payload,
            created_at: Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![model]])
            .into_connection();

        let result = latest_snapshot(&db, 1).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().tracking_id, 1);
    }

    #[tokio::test]
    async fn test_load_l2_snapshot_from_db_none() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![]])
            .into_connection();
        let result = load_l2_snapshot_from_db(&db, 1).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_load_l2_snapshot_from_db_parse_error() {
        let model = l2_snapshots::Model {
            id: 1,
            tracking_id: 1,
            snapshot_type: "l2".to_string(),
            checksum: "c".to_string(),
            payload: json!({"not": "collect_result"}),
            created_at: Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![model]])
            .into_connection();
        let result = load_l2_snapshot_from_db(&db, 1).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("解析 L2 快照 payload 失败"));
    }

    #[tokio::test]
    async fn test_collect_l2_commits_parses_cve_list() {
        use crate::entities::l2_commit_records;

        let model = l2_commit_records::Model {
            id: 1,
            tracking_id: 1,
            commit_sha: "sha".to_string(),
            commit_message: "m".to_string(),
            author_name: "a".to_string(),
            author_email: "e".to_string(),
            committed_at: Utc::now(),
            change_type: None,
            primary_change_type: None,
            cve_list: Some(json!(["CVE-2025-1"])),
            spec_changed: false,
            patch_stats: None,
            classification_status: "pending".to_string(),
            classification_notes: None,
            sync_status: "not_synced".to_string(),
            synced_to_l2_commit: None,
            synced_at: None,
            api_url: "u".to_string(),
            fetched_at: Utc::now(),
            files_changed_count: 0,
            additions: 0,
            deletions: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            spec_version: None,
            spec_release: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<l2_commit_records::Model, _, _>(vec![vec![model]])
            .into_connection();
        let commits = collect_l2_commits(&db, 1).await.unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].cve_list, vec!["CVE-2025-1".to_string()]);
    }

    #[tokio::test]
    async fn test_persist_l2_commits_inserts_and_handles_empty_cve() {
        use crate::entities::l2_commit_records;

        let commit = CommitEntry {
            sha: "sha".to_string(),
            title: "t".to_string(),
            message: "m".to_string(),
            author: "a".to_string(),
            authored_at: Utc::now(),
            url: None,
            stats: ChangeStats {
                additions: 1,
                deletions: 2,
                files_changed: 1,
            },
            primary_change_type: None,
            cve_list: vec![],
        };

        let inserted = l2_commit_records::Model {
            id: 1,
            tracking_id: 1,
            commit_sha: commit.sha.clone(),
            commit_message: commit.message.clone(),
            author_name: commit.author.clone(),
            author_email: String::new(),
            committed_at: commit.authored_at,
            change_type: None,
            primary_change_type: None,
            cve_list: None,
            spec_changed: false,
            patch_stats: None,
            classification_status: "pending".to_string(),
            classification_notes: None,
            sync_status: "not_synced".to_string(),
            synced_to_l2_commit: None,
            synced_at: None,
            api_url: String::new(),
            fetched_at: Utc::now(),
            files_changed_count: commit.stats.files_changed,
            additions: commit.stats.additions,
            deletions: commit.stats.deletions,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            spec_version: Some("1.0.0".to_string()),
            spec_release: Some("1".to_string()),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<l2_commit_records::Model, _, _>(vec![vec![inserted]])
            .into_connection();

        let result = persist_l2_commits(&db, 1, &[commit], Some("1.0.0"), Some("1")).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_spec_from_repo_no_spec() {
        let temp_dir = tempfile::tempdir().unwrap();
        let (v, r) = parse_spec_from_repo(temp_dir.path()).unwrap();
        assert!(v.is_none());
        assert!(r.is_none());
    }

    fn make_tracking_model(id: i32) -> tracking::Model {
        tracking::Model {
            id,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "o".to_string(),
            l1_repo_name: "r".to_string(),
            l2_branch: "b".to_string(),
            l2_repo_path: "/x".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: None,
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        }
    }

    #[tokio::test]
    async fn test_export_l2_snapshot_tracking_not_found() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<tracking::Model, _, _>(vec![vec![]])
            .into_connection();

        let result = export_l2_snapshot(
            &db,
            1,
            PathBuf::from("/tmp/track-system-metadata-bridge-test-repo"),
            PathBuf::from("/tmp/track-system-metadata-bridge-test.json"),
        )
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("tracking configuration not found"));
    }

    #[tokio::test]
    async fn test_build_repository_snapshot_repo_path_missing_bails() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let tracking_model = make_tracking_model(1);

        let result = build_repository_snapshot(
            &db,
            &tracking_model,
            SnapshotOrigin::L2,
            Some(Path::new("/tmp/track-system-repo-does-not-exist")),
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_build_repository_snapshot_l2_loads_from_db_when_no_repo_path() {
        let now = Utc::now();
        let payload = json!({
            "level": "l2",
            "platform": "gitee",
            "owner": null,
            "repo": "repo",
            "branch": "main",
            "collected_at": now.to_rfc3339(),
            "commits": [],
            "snapshot": null,
            "files": [{
                "path": "a.patch",
                "sha256": "h",
                "size": 0,
                "is_binary": false
            }],
            "spec": {
                "path": "pkg.spec",
                "version": "1.0.0",
                "release": "1",
                "content_base64": "",
                "sha256": "s"
            },
            "issues": [{
                "number": 1,
                "title": "t",
                "state": "open",
                "author": "a",
                "created_at": now.to_rfc3339(),
                "updated_at": now.to_rfc3339(),
                "closed_at": null,
                "labels": []
            }]
        });

        let snapshot_model = l2_snapshots::Model {
            id: 10,
            tracking_id: 1,
            snapshot_type: "l2".to_string(),
            checksum: "c".to_string(),
            payload,
            created_at: now,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![snapshot_model]])
            .into_connection();

        let tracking_model = make_tracking_model(1);
        let snapshot = build_repository_snapshot(&db, &tracking_model, SnapshotOrigin::L2, None)
            .await
            .unwrap();

        assert_eq!(snapshot.tracking_id, 1);
        assert_eq!(snapshot.origin, SnapshotOrigin::L2);
        assert!(snapshot.spec.is_some());
        assert_eq!(snapshot.files.len(), 1);
        assert_eq!(snapshot.issues.len(), 1);
        assert_eq!(snapshot.issues[0].number, "1");
    }

    #[test]
    fn test_collect_files_from_repo_walks_and_skips_git_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git").join("config"), b"ignored").unwrap();

        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("a.txt"), b"hello").unwrap();
        std::fs::write(root.join("src").join("b.bin"), [0xffu8, 0xfeu8, 0xfdu8]).unwrap();

        let files = collect_files(SnapshotOrigin::L2, None, Some(root)).unwrap();
        let paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();

        assert!(paths.iter().any(|p| p == "a.txt"));
        assert!(paths.iter().any(|p| p == "src/b.bin"));
        assert!(!paths.iter().any(|p| p.contains(".git")));

        let a = files.iter().find(|f| f.path == "a.txt").unwrap();
        assert!(!a.is_binary);
        let b = files.iter().find(|f| f.path == "src/b.bin").unwrap();
        assert!(b.is_binary);
    }

    #[tokio::test]
    async fn test_collect_spec_from_repo_finds_spec_and_extracts_version_release() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        std::fs::write(
            root.join("pkg.spec"),
            "Name: pkg\nVersion: 1.2.3\nRelease: 7%{?dist}\n",
        )
        .unwrap();

        let tracking_model = make_tracking_model(1);
        let got = collect_spec(SnapshotOrigin::L2, &tracking_model, Some(root))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(got.path, "pkg.spec");
        assert_eq!(got.version, Some("1.2.3".to_string()));
        assert_eq!(got.release, Some("7".to_string()));
    }

    #[tokio::test]
    async fn test_collect_spec_from_repo_returns_none_when_missing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let tracking_model = make_tracking_model(1);
        let got = collect_spec(SnapshotOrigin::L2, &tracking_model, Some(root))
            .await
            .unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn test_collect_commits_maps_models() {
        let now = Utc::now();
        let model = l1_commit_records::Model {
            id: 1,
            tracking_id: 1,
            commit_sha: "sha".to_string(),
            commit_message: "title\nbody".to_string(),
            author_name: "a".to_string(),
            author_email: "e".to_string(),
            committed_at: now,
            change_type: None,
            primary_change_type: Some("bugfix".to_string()),
            cve_list: Some(json!(["CVE-2025-1234"])),
            spec_changed: false,
            patch_stats: None,
            classification_status: "pending".to_string(),
            classification_notes: None,
            sync_status: "not_synced".to_string(),
            synced_to_l2_commit: None,
            synced_at: None,
            api_url: "u".to_string(),
            fetched_at: now,
            files_changed_count: 3,
            additions: 10,
            deletions: 2,
            created_at: now,
            updated_at: now,
            spec_version: None,
            spec_release: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<l1_commit_records::Model, _, _>(vec![vec![model]])
            .into_connection();

        let commits = collect_commits(&db, 1).await.unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].sha, "sha");
        assert_eq!(commits[0].title, "title");
        assert_eq!(commits[0].cve_list, vec!["CVE-2025-1234".to_string()]);
        assert_eq!(commits[0].stats.additions, 10);
        assert_eq!(commits[0].stats.deletions, 2);
        assert_eq!(commits[0].stats.files_changed, 3);
    }

    #[tokio::test]
    async fn test_collect_issues_parses_labels_from_string_or_array() {
        let now = Utc::now();
        let model_newer = issues::Model {
            id: 2,
            tracking_id: 1,
            issue_number: "124".to_string(),
            title: "t2".to_string(),
            state: "open".to_string(),
            author: "a2".to_string(),
            api_url: "u2".to_string(),
            labels: Some(json!("single")),
            created_at: now,
            updated_at: now,
            closed_at: None,
            raw_payload: None,
        };
        let model_older = issues::Model {
            id: 1,
            tracking_id: 1,
            issue_number: "123".to_string(),
            title: "t1".to_string(),
            state: "open".to_string(),
            author: "a1".to_string(),
            api_url: "u1".to_string(),
            labels: Some(json!(["l1", "l2"])),
            created_at: now,
            updated_at: now - chrono::Duration::seconds(1),
            closed_at: None,
            raw_payload: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<issues::Model, _, _>(vec![vec![model_newer, model_older]])
            .into_connection();

        let got = collect_issues(&db, 1).await.unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].number, "124");
        assert_eq!(got[0].labels, vec!["single".to_string()]);
        assert_eq!(got[1].number, "123");
        assert_eq!(got[1].labels, vec!["l1".to_string(), "l2".to_string()]);
    }

    #[tokio::test]
    async fn test_persist_snapshot_writes_file_and_returns_summary_for_all_origins() {
        let now = Utc::now();
        let file_entry = FileEntry {
            path: "a.txt".to_string(),
            size: 1,
            sha256: "h".to_string(),
            is_binary: false,
        };
        let commit_entry = CommitEntry {
            sha: "sha".to_string(),
            title: "t".to_string(),
            message: "m".to_string(),
            author: "a".to_string(),
            authored_at: now,
            url: None,
            stats: ChangeStats {
                additions: 1,
                deletions: 0,
                files_changed: 1,
            },
            primary_change_type: None,
            cve_list: vec![],
        };
        let issue_entry = IssueEntry {
            number: "1".to_string(),
            title: "i".to_string(),
            state: "open".to_string(),
            author: "u".to_string(),
            labels: vec![],
            updated_at: now,
        };

        let tmp = tempfile::tempdir().unwrap();
        let p_l1 = tmp.path().join("l1.json");
        let p_l2 = tmp.path().join("l2.json");
        let p_u = tmp.path().join("u.json");

        let m1 = l2_snapshots::Model {
            id: 1,
            tracking_id: 1,
            snapshot_type: "l1".to_string(),
            checksum: "c1".to_string(),
            payload: json!({}),
            created_at: now,
        };
        let m2 = l2_snapshots::Model {
            id: 2,
            tracking_id: 1,
            snapshot_type: "l2".to_string(),
            checksum: "c2".to_string(),
            payload: json!({}),
            created_at: now,
        };
        let m3 = l2_snapshots::Model {
            id: 3,
            tracking_id: 1,
            snapshot_type: "unknown".to_string(),
            checksum: "c3".to_string(),
            payload: json!({}),
            created_at: now,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![m1]])
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![m2]])
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![m3]])
            .into_connection();

        for (origin, path) in [
            (SnapshotOrigin::L1, &p_l1),
            (SnapshotOrigin::L2, &p_l2),
            (SnapshotOrigin::Unknown, &p_u),
        ] {
            let mut snapshot = RepositorySnapshot::new(1, origin);
            snapshot.files = vec![file_entry.clone()];
            snapshot.commits = vec![commit_entry.clone()];
            snapshot.issues = vec![issue_entry.clone()];
            snapshot.spec = Some(SpecEntry {
                path: "pkg.spec".to_string(),
                version: Some("1.0.0".to_string()),
                release: Some("1".to_string()),
