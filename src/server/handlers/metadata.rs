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
