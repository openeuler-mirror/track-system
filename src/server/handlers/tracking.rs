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

//! 跟踪配置管理 API handlers

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::Utc;
use sea_orm::*;
use serde::{Deserialize, Serialize};

use crate::{
    entities::{prelude::*, tracking},
    server::{
        api::{ApiResponse, PaginatedResponse},
        error::{ApiError, ApiResult},
        state::AppState,
    },
};

/// 跟踪配置列表查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingListQuery {
    /// 页码（从 1 开始）
    pub page: Option<u64>,
    /// 每页大小
    pub page_size: Option<u64>,
    /// 按软件包 ID 过滤
    pub package_id: Option<i32>,
    /// 按发行版 ID 过滤
    pub distro_id: Option<i32>,
    /// 按状态过滤
    pub tracking_status: Option<String>,
}

/// 创建跟踪配置请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTrackingRequest {
    /// 软件包 ID
    pub package_id: i32,
    /// 发行版 ID
    pub distro_id: i32,
    /// L1 仓库所有者
    pub l1_repo_owner: String,
    /// L1 仓库名称
    pub l1_repo_name: String,
    /// L1 分支
    pub l1_branch: String,
    /// L2 分支
    pub l2_branch: String,
    /// L2 仓库路径
    pub l2_repo_path: String,
    /// 跟踪状态
    pub tracking_status: Option<String>,
}

/// 更新跟踪配置请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTrackingRequest {
    /// L1 仓库所有者
    pub l1_repo_owner: Option<String>,
    /// L1 仓库名称
    pub l1_repo_name: Option<String>,
    /// L1 分支
    pub l1_branch: Option<String>,
    /// L2 分支
    pub l2_branch: Option<String>,
    /// L2 仓库路径
    pub l2_repo_path: Option<String>,
    /// 跟踪状态
    pub tracking_status: Option<String>,
}

/// 跟踪配置响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingResponse {
    pub id: i32,
    pub package_id: i32,
    pub distro_id: i32,
    pub l1_repo_owner: String,
    pub l1_repo_name: String,
    pub l1_branch: String,
    pub l2_branch: String,
    pub l2_repo_path: String,
    pub tracking_status: String,
    pub last_sync_time: Option<chrono::DateTime<chrono::Utc>>,
    pub last_l1_commit_sha: Option<String>,
    pub last_l2_commit_sha: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<tracking::Model> for TrackingResponse {
    fn from(model: tracking::Model) -> Self {
        Self {
            id: model.id,
            package_id: model.package_id,
            distro_id: model.distro_id,
            l1_repo_owner: model.l1_repo_owner,
            l1_repo_name: model.l1_repo_name,
            l1_branch: model.l1_branch,
            l2_branch: model.l2_branch,
            l2_repo_path: model.l2_repo_path,
            tracking_status: model.tracking_status,
            last_sync_time: model.last_sync_time,
            last_l1_commit_sha: model.last_l1_commit_sha,
            last_l2_commit_sha: model.last_l2_commit_sha,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

/// GET /api/tracking
///
/// 查询跟踪配置列表（支持分页和过滤）
pub async fn list_tracking(
    State(state): State<AppState>,
    Query(query): Query<TrackingListQuery>,
) -> ApiResult<Json<ApiResponse<PaginatedResponse<TrackingResponse>>>> {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(10);

    // 验证分页参数
