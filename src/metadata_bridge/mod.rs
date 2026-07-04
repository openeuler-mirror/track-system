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
