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

//! 元数据管理 API handlers

use axum::{
    extract::{Path, Query, State},
    Json,
};
use sea_orm::ActiveModelTrait;
use serde::{Deserialize, Serialize};

use crate::{
    server::{
        api::ApiResponse,
        error::{ApiError, ApiResult},
        state::AppState,
    },
    snapshot::types::RepositorySnapshot,
};

/// L0 元数据导入请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportL0Request {
    /// 跟踪配置 ID
    pub tracking_id: i32,
    /// 仓库快照数据
    pub snapshot: RepositorySnapshot,
}

/// L1 元数据导入请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportL1Request {
    /// 跟踪配置 ID
    pub tracking_id: i32,
    /// 仓库快照数据
    pub snapshot: RepositorySnapshot,
}

/// L2 元数据导入请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportL2Request {
    /// 跟踪配置 ID
    pub tracking_id: i32,
    /// 仓库快照数据
    pub snapshot: RepositorySnapshot,
}

/// 元数据导入响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResponse {
    /// 导入的快照 ID
    pub snapshot_id: String,
    /// 跟踪配置 ID
    pub tracking_id: i32,
    /// 文件数量
    pub file_count: usize,
    /// 导入时间
    pub imported_at: chrono::DateTime<chrono::Utc>,
}

/// POST /api/metadata/l0
///
/// 导入 L0（上游社区）元数据
pub async fn import_l0_metadata(
    State(state): State<AppState>,
    Json(request): Json<ImportL0Request>,
) -> ApiResult<Json<ApiResponse<ImportResponse>>> {
    use crate::entities::prelude::*;
    use sea_orm::{EntityTrait, Set};

    // 验证请求
    validate_import_request(request.tracking_id, &request.snapshot)?;

    // 1. 验证 tracking_id 存在
    let tracking = Tracking::find_by_id(request.tracking_id)
        .one(state.db.as_ref())
        .await
        .map_err(ApiError::DatabaseError)?
        .ok_or_else(|| ApiError::NotFound(format!("跟踪配置 {} 不存在", request.tracking_id)))?;

    // 获取关联的 package_id
    let package_id = tracking.package_id;

    // 2. 保存快照到数据库（L0 使用 l0_commits 表）
    let mut commits_imported = 0;
    let mut commits_skipped = 0;

    for commit in &request.snapshot.commits {
        match import_l0_commit(state.db.as_ref(), package_id, commit).await {
            Ok(true) => commits_imported += 1,
            Ok(false) => commits_skipped += 1,
            Err(e) => {
                tracing::warn!("导入 L0 commit {} 失败: {}", commit.sha, e);
                commits_skipped += 1;
            }
        }
    }

    // 3. 更新跟踪配置的最后同步时间
    let now = chrono::Utc::now();
    let mut tracking_active: crate::entities::tracking::ActiveModel = tracking.into();
    tracking_active.last_sync_time = Set(Some(now));
    tracking_active.updated_at = Set(now);
    tracking_active
        .update(state.db.as_ref())
        .await
        .map_err(ApiError::DatabaseError)?;

    // 4. 触发对比任务（可选）
    if let Err(e) = trigger_comparison_task(&state, request.tracking_id).await {
        tracing::warn!("触发对比任务失败: {}", e);
    }

    // 生成快照 ID（使用时间戳和 tracking_id）
    let snapshot_id = format!("l0-{}-{}", request.tracking_id, now.timestamp());

    tracing::info!(
        "L0 元数据导入完成: snapshot_id={}, commits_imported={}, commits_skipped={}",
        snapshot_id,
        commits_imported,
        commits_skipped
    );

    let response = ImportResponse {
        snapshot_id,
        tracking_id: request.tracking_id,
        file_count: request.snapshot.files.len(),
        imported_at: now,
    };

    Ok(Json(ApiResponse::created(response)))
}

/// POST /api/metadata/l1
///
/// 导入 L1（发行版）元数据
pub async fn import_l1_metadata(
    State(state): State<AppState>,
    Json(request): Json<ImportL1Request>,
) -> ApiResult<Json<ApiResponse<ImportResponse>>> {
    use crate::entities::prelude::*;
    use sea_orm::{EntityTrait, Set};

    // 验证请求
    validate_import_request(request.tracking_id, &request.snapshot)?;

    // 1. 验证 tracking_id 存在
    let tracking = Tracking::find_by_id(request.tracking_id)
        .one(state.db.as_ref())
        .await
        .map_err(ApiError::DatabaseError)?
        .ok_or_else(|| ApiError::NotFound(format!("跟踪配置 {} 不存在", request.tracking_id)))?;

    // 2. 保存快照到数据库（L1 使用 commit_records 和 issues 表）
    let mut commits_imported = 0;
    let mut commits_skipped = 0;
    let mut issues_imported = 0;
    let mut issues_skipped = 0;

    // 导入 commits
    for commit in &request.snapshot.commits {
        match import_commit_record(state.db.as_ref(), request.tracking_id, commit).await {
            Ok(true) => commits_imported += 1,
            Ok(false) => commits_skipped += 1,
            Err(e) => {
                tracing::warn!("导入 L1 commit {} 失败: {}", commit.sha, e);
                commits_skipped += 1;
            }
        }
    }

    // 导入 issues
    for issue in &request.snapshot.issues {
        match import_issue(state.db.as_ref(), request.tracking_id, issue).await {
            Ok(true) => issues_imported += 1,
            Ok(false) => issues_skipped += 1,
            Err(e) => {
                tracing::warn!("导入 L1 issue {} 失败: {}", issue.number, e);
                issues_skipped += 1;
            }
        }
    }

    // 3. 更新跟踪配置的最后同步时间
    let now = chrono::Utc::now();
    let mut tracking_active: crate::entities::tracking::ActiveModel = tracking.into();
    tracking_active.last_sync_time = Set(Some(now));
    tracking_active.updated_at = Set(now);
    tracking_active
        .update(state.db.as_ref())
        .await
        .map_err(ApiError::DatabaseError)?;

    // 4. 触发 L1 vs L0 对比任务（可选）
    if let Err(e) = trigger_comparison_task(&state, request.tracking_id).await {
        tracing::warn!("触发对比任务失败: {}", e);
    }

    // 生成快照 ID（使用时间戳和 tracking_id）
    let snapshot_id = format!("l1-{}-{}", request.tracking_id, now.timestamp());

    tracing::info!(
        "L1 元数据导入完成: snapshot_id={}, commits={}/{}, issues={}/{}",
        snapshot_id,
        commits_imported,
        commits_imported + commits_skipped,
        issues_imported,
        issues_imported + issues_skipped
    );

    let response = ImportResponse {
        snapshot_id,
        tracking_id: request.tracking_id,
        file_count: request.snapshot.files.len(),
        imported_at: now,
    };

    Ok(Json(ApiResponse::created(response)))
}

/// POST /api/metadata/l2
///
/// 导入 L2（企业发行版）元数据
pub async fn import_l2_metadata(
    State(state): State<AppState>,
    Json(request): Json<ImportL2Request>,
) -> ApiResult<Json<ApiResponse<ImportResponse>>> {
    use crate::entities::prelude::*;
    use sea_orm::{EntityTrait, Set};

    // 验证请求
    validate_import_request(request.tracking_id, &request.snapshot)?;

    // 1. 验证 tracking_id 存在
    let tracking = Tracking::find_by_id(request.tracking_id)
        .one(state.db.as_ref())
        .await
        .map_err(ApiError::DatabaseError)?
        .ok_or_else(|| ApiError::NotFound(format!("跟踪配置 {} 不存在", request.tracking_id)))?;

    // 2. 保存快照到数据库（L2 使用 l2_snapshots 表存储完整快照）
    let snapshot_json = serde_json::to_value(&request.snapshot)
        .map_err(|e| ApiError::BadRequest(format!("序列化快照失败: {}", e)))?;

    // 计算快照校验和
    let checksum = calculate_snapshot_checksum(&snapshot_json);

    let now = chrono::Utc::now();
    let snapshot_id = format!("l2-{}-{}", request.tracking_id, now.timestamp());

    let l2_snapshot = crate::entities::l2_snapshots::ActiveModel {
        tracking_id: Set(request.tracking_id),
        snapshot_type: Set("l2".to_string()),
        checksum: Set(checksum.clone()),
        payload: Set(snapshot_json),
        created_at: Set(now),
        ..Default::default()
    };

    let inserted_snapshot = L2Snapshots::insert(l2_snapshot)
        .exec(state.db.as_ref())
        .await
        .map_err(ApiError::DatabaseError)?;

    tracing::info!(
        "L2 快照已保存: id={}, tracking_id={}, checksum={}",
        inserted_snapshot.last_insert_id,
        request.tracking_id,
        checksum
    );

    // 3. 更新跟踪配置的最后同步时间
    let mut tracking_active: crate::entities::tracking::ActiveModel = tracking.into();
    tracking_active.last_sync_time = Set(Some(now));
    tracking_active.updated_at = Set(now);
    tracking_active
        .update(state.db.as_ref())
        .await
        .map_err(ApiError::DatabaseError)?;

    // 4. 触发 L2 vs L1 对比任务（可选）
    if let Err(e) = trigger_comparison_task(&state, request.tracking_id).await {
        tracing::warn!("触发对比任务失败: {}", e);
    }

    tracing::info!(
        "L2 元数据导入完成: snapshot_id={}, files={}, commits={}, issues={}",
        snapshot_id,
        request.snapshot.files.len(),
        request.snapshot.commits.len(),
        request.snapshot.issues.len()
    );

    let response = ImportResponse {
        snapshot_id,
        tracking_id: request.tracking_id,
        file_count: request.snapshot.files.len(),
        imported_at: now,
    };

    Ok(Json(ApiResponse::created(response)))
}

/// GET /api/metadata/l0
///
/// 列出 L0 元数据快照
pub async fn list_l0_metadata(
    State(_state): State<AppState>,
    Query(query): Query<MetadataListQuery>,
) -> ApiResult<Json<ApiResponse<Vec<MetadataSummary>>>> {
    // TODO: 实现 L0 元数据列表查询
    // 1. 从数据库查询 L0 快照列表
    // 2. 应用过滤条件（tracking_id）
    // 3. 返回结果

    let _tracking_id = query.tracking_id;

    // 临时实现：返回空列表
    Ok(Json(ApiResponse::success(vec![])))
}

/// GET /api/metadata/l0/:id
///
/// 获取 L0 元数据详情
pub async fn get_l0_metadata(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ApiResponse<MetadataDetail>>> {
    // TODO: 实现 L0 元数据详情查询
    // 1. 从数据库查询指定 ID 的 L0 快照
    // 2. 如果不存在，返回 404
    // 3. 返回快照详情

    // 临时实现：返回模拟数据
    let detail = MetadataDetail {
        id: id.clone(),
        tracking_id: 1,
        level: "l0".to_string(),
        file_count: 0,
        imported_at: chrono::Utc::now(),
    };

    Ok(Json(ApiResponse::success(detail)))
}

pub async fn delete_l0_metadata(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<axum::http::StatusCode> {
    // TODO: 实现 L0 元数据删除
    // 1. 查找指定 ID 的 L0 快照
    // 2. 如果不存在，返回 404
    // 3. 删除快照及相关数据
    // 4. 返回 204 No Content

    tracing::info!(snapshot_id = %id, "删除 L0 元数据");

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// GET /api/metadata/l1
///
/// 列出 L1 元数据快照
pub async fn list_l1_metadata(
    State(_state): State<AppState>,
    Query(query): Query<MetadataListQuery>,
) -> ApiResult<Json<ApiResponse<Vec<MetadataSummary>>>> {
    let _tracking_id = query.tracking_id;
    Ok(Json(ApiResponse::success(vec![])))
}

/// GET /api/metadata/l1/:id
///
/// 获取 L1 元数据详情
pub async fn get_l1_metadata(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ApiResponse<MetadataDetail>>> {
    let detail = MetadataDetail {
        id: id.clone(),
        tracking_id: 1,
        level: "l1".to_string(),
        file_count: 0,
        imported_at: chrono::Utc::now(),
    };
    Ok(Json(ApiResponse::success(detail)))
}

pub async fn delete_l1_metadata(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<axum::http::StatusCode> {
    tracing::info!(snapshot_id = %id, "删除 L1 元数据");
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// GET /api/metadata/l2
///
/// 列出 L2 元数据快照
pub async fn list_l2_metadata(
    State(_state): State<AppState>,
    Query(query): Query<MetadataListQuery>,
) -> ApiResult<Json<ApiResponse<Vec<MetadataSummary>>>> {
    let _tracking_id = query.tracking_id;
    Ok(Json(ApiResponse::success(vec![])))
}

/// GET /api/metadata/l2/:id
///
/// 获取 L2 元数据详情
pub async fn get_l2_metadata(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ApiResponse<MetadataDetail>>> {
    let detail = MetadataDetail {
        id: id.clone(),
        tracking_id: 1,
        level: "l2".to_string(),
        file_count: 0,
        imported_at: chrono::Utc::now(),
    };
    Ok(Json(ApiResponse::success(detail)))
}

pub async fn delete_l2_metadata(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<axum::http::StatusCode> {
    tracing::info!(snapshot_id = %id, "删除 L2 元数据");
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// 元数据列表查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataListQuery {
    /// 按跟踪配置 ID 过滤
    pub tracking_id: Option<i32>,
}

/// 元数据摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataSummary {
    /// 快照 ID
    pub id: String,
    /// 跟踪配置 ID
    pub tracking_id: i32,
    /// 层级（l0, l1, l2）
    pub level: String,
    /// 文件数量
    pub file_count: usize,
    /// 导入时间
    pub imported_at: chrono::DateTime<chrono::Utc>,
}

/// 元数据详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataDetail {
    /// 快照 ID
    pub id: String,
    /// 跟踪配置 ID
    pub tracking_id: i32,
    /// 层级（l0, l1, l2）
    pub level: String,
    /// 文件数量
    pub file_count: usize,
    /// 导入时间
    pub imported_at: chrono::DateTime<chrono::Utc>,
}

/// 验证导入请求
fn validate_import_request(tracking_id: i32, snapshot: &RepositorySnapshot) -> ApiResult<()> {
    // 验证 tracking_id
    if tracking_id <= 0 {
        return Err(ApiError::BadRequest("Invalid tracking_id".to_string()));
    }

    // 验证 tracking_id 匹配
    if snapshot.tracking_id != tracking_id {
        return Err(ApiError::BadRequest(format!(
            "Snapshot tracking_id ({}) does not match request tracking_id ({})",
            snapshot.tracking_id, tracking_id
        )));
    }

    Ok(())
}

/// 导入 L0 commit 到数据库
async fn import_l0_commit(
    db: &sea_orm::DatabaseConnection,
    package_id: i32,
    commit: &crate::snapshot::types::CommitEntry,
) -> anyhow::Result<bool> {
    use crate::entities::{l0_commits, prelude::*};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};

    // 检查是否已存在
    let existing = L0Commits::find()
        .filter(l0_commits::Column::PackageId.eq(package_id))
        .filter(l0_commits::Column::CommitSha.eq(&commit.sha))
        .one(db)
        .await?;

    if existing.is_some() {
        return Ok(false); // 已存在，跳过
    }

    // 构建 metadata JSON
    let metadata = serde_json::json!({
        "title": commit.title,
        "message": commit.message,
        "url": commit.url,
        "stats": commit.stats,
        "primary_change_type": commit.primary_change_type,
        "cve_list": commit.cve_list,
    });

    // 插入新记录
    let new_commit = l0_commits::ActiveModel {
        package_id: Set(package_id),
        repo: Set(commit.url.clone().unwrap_or_default()),
        commit_sha: Set(commit.sha.clone()),
        summary: Set(commit.title.clone()),
        authored_at: Set(commit.authored_at),
        metadata: Set(Some(metadata)),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
        ..Default::default()
    };

    L0Commits::insert(new_commit).exec(db).await?;
    Ok(true)
}

/// 导入 commit record 到数据库（L1）
async fn import_commit_record(
    db: &sea_orm::DatabaseConnection,
    tracking_id: i32,
    commit: &crate::snapshot::types::CommitEntry,
) -> anyhow::Result<bool> {
    use crate::entities::{l1_commit_records, prelude::*};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};

    // 检查是否已存在
    let existing = L1CommitRecords::find()
        .filter(l1_commit_records::Column::TrackingId.eq(tracking_id))
        .filter(l1_commit_records::Column::CommitSha.eq(&commit.sha))
        .one(db)
        .await?;

    if existing.is_some() {
        return Ok(false); // 已存在，跳过
    }

    // 转换 CVE 列表
    let cve_list_json = if !commit.cve_list.is_empty() {
        Some(serde_json::to_value(&commit.cve_list)?)
    } else {
        None
    };

    // 插入新记录
    let new_commit = l1_commit_records::ActiveModel {
        tracking_id: Set(tracking_id),
        commit_sha: Set(commit.sha.clone()),
        commit_message: Set(commit.message.clone()),
        author_name: Set(commit.author.clone()),
        author_email: Set(String::new()), // 从快照中无法获取
        committed_at: Set(commit.authored_at),
        change_type: Set(None),
        primary_change_type: Set(commit.primary_change_type.clone()),
        cve_list: Set(cve_list_json),
        spec_changed: Set(false),
        patch_stats: Set(None),
        classification_status: Set("unclassified".to_string()),
        classification_notes: Set(None),
        sync_status: Set("not_synced".to_string()),
        synced_to_l2_commit: Set(None),
        synced_at: Set(None),
        api_url: Set(commit.url.clone().unwrap_or_default()),
        fetched_at: Set(chrono::Utc::now()),
        files_changed_count: Set(commit.stats.files_changed),
        additions: Set(commit.stats.additions),
        deletions: Set(commit.stats.deletions),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
        ..Default::default()
    };

    L1CommitRecords::insert(new_commit).exec(db).await?;
    Ok(true)
}

/// 导入 issue 到数据库
async fn import_issue(
    db: &sea_orm::DatabaseConnection,
    tracking_id: i32,
    issue: &crate::snapshot::types::IssueEntry,
) -> anyhow::Result<bool> {
    use crate::entities::{issues, prelude::*};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};

    // 检查是否已存在
    let existing = Issues::find()
        .filter(issues::Column::TrackingId.eq(tracking_id))
        .filter(issues::Column::IssueNumber.eq(&issue.number))
        .one(db)
        .await?;

    if existing.is_some() {
        return Ok(false); // 已存在，跳过
    }

    // 转换 labels
    let labels_json = if !issue.labels.is_empty() {
        Some(serde_json::to_value(&issue.labels)?)
    } else {
        None
    };

    // 插入新记录
    let new_issue = issues::ActiveModel {
        tracking_id: Set(tracking_id),
        issue_number: Set(issue.number.clone()),
        title: Set(issue.title.clone()),
        state: Set(issue.state.clone()),
        author: Set(issue.author.clone()),
        api_url: Set(String::new()), // 从快照中无法获取
        labels: Set(labels_json),
        created_at: Set(issue.updated_at), // 使用 updated_at 作为创建时间
        updated_at: Set(issue.updated_at),
        closed_at: Set(None),
        raw_payload: Set(None),
        ..Default::default()
    };

    Issues::insert(new_issue).exec(db).await?;
    Ok(true)
}

/// 计算快照校验和
fn calculate_snapshot_checksum(snapshot_json: &serde_json::Value) -> String {
    use sha2::{Digest, Sha256};
    let json_str = serde_json::to_string(snapshot_json).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(json_str.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 触发对比任务
async fn trigger_comparison_task(state: &AppState, tracking_id: i32) -> anyhow::Result<()> {
    // 使用 SyncManager 入队同步作业
    let sync_manager = state.scheduler();
    sync_manager.queue_sync_job(tracking_id, 0).await?;
    tracing::info!("已为 tracking_id={} 入队同步作业", tracking_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{issues, l0_commits, l1_commit_records, sync_jobs};
    use chrono::Utc;
    use sea_orm::{DatabaseBackend, MockDatabase};
    use std::sync::Once;

    fn init_test_tracing() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let filter =
                tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                    tracing_subscriber::EnvFilter::new("track_system=debug,sea_orm=info")
                });

            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_test_writer()
                .try_init()
                .ok();
        });
    }

    fn create_test_snapshot(tracking_id: i32) -> RepositorySnapshot {
        RepositorySnapshot {
            tracking_id,
            origin: crate::snapshot::types::SnapshotOrigin::L1,
            generated_at: Utc::now(),
            spec: None,
            files: vec![],
            commits: vec![],
            issues: vec![],
        }
    }

    #[test]
    fn test_validate_import_request_success() {
        let snapshot = create_test_snapshot(1);
        let result = validate_import_request(1, &snapshot);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_import_request_invalid_tracking_id() {
        let snapshot = create_test_snapshot(1);
        let result = validate_import_request(0, &snapshot);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_import_request_mismatched_tracking_id() {
        let snapshot = create_test_snapshot(1);
        let result = validate_import_request(2, &snapshot);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_import_l0_metadata_invalid_tracking() {
        use sea_orm::{DatabaseBackend, MockDatabase};
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<crate::entities::tracking::Model, _, _>([vec![]])
            .into_connection();
        let state = AppState::without_external_clients(db);

        let snapshot = create_test_snapshot(999);
        let request = ImportL0Request {
            tracking_id: 999,
            snapshot,
        };

        let result = import_l0_metadata(State(state), Json(request)).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::NotFound(_)));
    }

    fn create_commit_entry(sha: &str) -> crate::snapshot::types::CommitEntry {
        crate::snapshot::types::CommitEntry {
            sha: sha.to_string(),
            title: "title".to_string(),
            message: "message".to_string(),
            author: "author".to_string(),
            authored_at: Utc::now(),
            url: Some("https://example.com/commit".to_string()),
            stats: crate::snapshot::types::ChangeStats {
                additions: 1,
                deletions: 1,
                files_changed: 1,
            },
            primary_change_type: Some("feature".to_string()),
            cve_list: vec!["CVE-2024-0001".to_string()],
        }
    }

    fn create_issue_entry(number: &str) -> crate::snapshot::types::IssueEntry {
        crate::snapshot::types::IssueEntry {
            number: number.to_string(),
            title: "issue title".to_string(),
            state: "open".to_string(),
            author: "author".to_string(),
            labels: vec!["bug".to_string()],
            updated_at: Utc::now(),
        }
    }

    fn create_tracking_model(id: i32, package_id: i32) -> crate::entities::tracking::Model {
        crate::entities::tracking::Model {
            id,
            package_id,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: None,
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        }
    }

    fn create_sync_job_model(id: i64, tracking_id: i32) -> sync_jobs::Model {
        sync_jobs::Model {
            id,
            tracking_id,
            job_kind: "sync".to_string(),
            scheduled_at: Utc::now(),
            started_at: None,
            finished_at: None,
            status: "pending".to_string(),
            error: None,
            attempt_count: 0,
            priority: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_l0_commit_model(id: i64, package_id: i32, sha: &str) -> l0_commits::Model {
        l0_commits::Model {
            id,
            package_id,
            repo: "https://example.com/commit".to_string(),
            commit_sha: sha.to_string(),
            summary: "title".to_string(),
            authored_at: Utc::now(),
            metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_l1_commit_record_model(
        id: i32,
        tracking_id: i32,
        sha: &str,
    ) -> l1_commit_records::Model {
        l1_commit_records::Model {
            id,
            tracking_id,
            commit_sha: sha.to_string(),
            commit_message: "message".to_string(),
            author_name: "author".to_string(),
            author_email: String::new(),
            committed_at: Utc::now(),
            change_type: None,
            primary_change_type: Some("feature".to_string()),
            cve_list: None,
            spec_changed: false,
            patch_stats: None,
            classification_status: "unclassified".to_string(),
            classification_notes: None,
            sync_status: "not_synced".to_string(),
            synced_to_l2_commit: None,
            synced_at: None,
            api_url: "https://example.com/commit".to_string(),
            fetched_at: Utc::now(),
            files_changed_count: 1,
            additions: 1,
            deletions: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            spec_version: None,
            spec_release: None,
        }
    }

    fn create_issue_model(id: i32, tracking_id: i32, number: &str) -> issues::Model {
        issues::Model {
            id,
            tracking_id,
            issue_number: number.to_string(),
            title: "issue title".to_string(),
            state: "open".to_string(),
            author: "author".to_string(),
