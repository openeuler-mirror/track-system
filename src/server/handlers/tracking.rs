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
    if page < 1 {
        return Err(ApiError::BadRequest("Page must be >= 1".to_string()));
    }
    if !(1..=100).contains(&page_size) {
        return Err(ApiError::BadRequest(
            "Page size must be between 1 and 100".to_string(),
        ));
    }

    // 构建查询
    let mut query_builder = Tracking::find();

    // 应用过滤条件
    if let Some(package_id) = query.package_id {
        query_builder = query_builder.filter(tracking::Column::PackageId.eq(package_id));
    }
    // if let Some(distro_id) = query.distro_id {
    //     query_builder = query_builder.filter(tracking::Column::DistroId.eq(distro_id));
    // }
    if let Some(tracking_status) = query.tracking_status {
        query_builder = query_builder.filter(tracking::Column::TrackingStatus.eq(tracking_status));
    }

    // 查询总数
    let total = query_builder.clone().count(state.db.as_ref()).await?;

    // 分页查询
    let tracking_list = query_builder
        .offset((page - 1) * page_size)
        .limit(page_size)
        .all(state.db.as_ref())
        .await?;

    let responses: Vec<TrackingResponse> = tracking_list.into_iter().map(Into::into).collect();
    let paginated = PaginatedResponse::new(responses, total, page, page_size);

    Ok(Json(ApiResponse::success(paginated)))
}

/// POST /api/tracking
///
/// 创建跟踪配置
pub async fn create_tracking(
    State(state): State<AppState>,
    Json(req): Json<CreateTrackingRequest>,
) -> ApiResult<Json<ApiResponse<TrackingResponse>>> {
    // 验证请求
    if req.package_id <= 0 {
        return Err(ApiError::BadRequest("Invalid package_id".to_string()));
    }
    if req.l1_repo_owner.is_empty() {
        return Err(ApiError::BadRequest(
            "l1_repo_owner is required".to_string(),
        ));
    }
    if req.l1_repo_name.is_empty() {
        return Err(ApiError::BadRequest("l1_repo_name is required".to_string()));
    }

    // 检查 package 是否存在
    let _package = Packages::find_by_id(req.package_id)
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Package {} not found", req.package_id)))?;

    println!(
        "Creating tracking for package {} and distro {} and l1_repo_owner {} 
               and l1_repo_name {} and l1_branch {} and l2_branch {} and l2_repo_path {}",
        req.package_id,
        req.distro_id,
        req.l1_repo_owner,
        req.l1_repo_name,
        req.l1_branch,
        req.l2_branch,
        req.l2_repo_path
    );
    // // 检查 distro 是否存在，避免外键约束错误
    // let _distro = Distros::find_by_id(req.distro_id)
    //     .one(state.db.as_ref())
    //     .await?
    //     .ok_or_else(|| ApiError::NotFound(format!("Distro {} not found", req.distro_id)))?;

    let now = Utc::now();
    let tracking = tracking::ActiveModel {
        package_id: Set(req.package_id),
        distro_id: Set(req.distro_id),
        l1_repo_owner: Set(req.l1_repo_owner),
        l1_repo_name: Set(req.l1_repo_name),
        l1_branch: Set(req.l1_branch),
        l2_branch: Set(req.l2_branch),
        l2_repo_path: Set(req.l2_repo_path),
        tracking_status: Set(req.tracking_status.unwrap_or_else(|| "active".to_string())),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };

    let result = tracking.insert(state.db.as_ref()).await?;

    Ok(Json(ApiResponse::created(result.into())))
}

/// GET /api/tracking/:id
///
/// 获取跟踪配置详情
pub async fn get_tracking(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<Json<ApiResponse<TrackingResponse>>> {
    let tracking = Tracking::find_by_id(id)
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Tracking {} not found", id)))?;

    Ok(Json(ApiResponse::success(tracking.into())))
}

/// PUT /api/tracking/:id
///
/// 更新跟踪配置
pub async fn update_tracking(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<UpdateTrackingRequest>,
) -> ApiResult<Json<ApiResponse<TrackingResponse>>> {
    // 查找现有配置
    let tracking = Tracking::find_by_id(id)
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Tracking {} not found", id)))?;

    let mut active: tracking::ActiveModel = tracking.into();

    // 更新字段
    if let Some(l1_repo_owner) = req.l1_repo_owner {
        active.l1_repo_owner = Set(l1_repo_owner);
    }
    if let Some(l1_repo_name) = req.l1_repo_name {
        active.l1_repo_name = Set(l1_repo_name);
    }
    if let Some(l1_branch) = req.l1_branch {
        active.l1_branch = Set(l1_branch);
    }
    if let Some(l2_branch) = req.l2_branch {
        active.l2_branch = Set(l2_branch);
    }
    if let Some(l2_repo_path) = req.l2_repo_path {
        active.l2_repo_path = Set(l2_repo_path);
    }
    if let Some(tracking_status) = req.tracking_status {
        active.tracking_status = Set(tracking_status);
    }

    active.updated_at = Set(Utc::now());

    let result = active.update(state.db.as_ref()).await?;

    Ok(Json(ApiResponse::success(result.into())))
}

/// DELETE /api/tracking/:id
///
/// 删除跟踪配置
pub async fn delete_tracking(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<Json<ApiResponse<()>>> {
    // 查找现有配置
    let tracking = Tracking::find_by_id(id)
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Tracking {} not found", id)))?;

    // 删除
    let active: tracking::ActiveModel = tracking.into();
    active.delete(state.db.as_ref()).await?;

    Ok(Json(ApiResponse::<()>::no_content()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{packages, tracking};
    use axum::extract::{Path, State};
    use sea_orm::{DatabaseBackend, MockDatabase};

    fn create_mock_tracking(id: i32, package_id: i32) -> tracking::Model {
        tracking::Model {
            id,
            package_id,
            distro_id: 1,
            l1_repo_owner: "openeuler".to_string(),
            l1_repo_name: "glibc".to_string(),
            l1_branch: "main".to_string(),
            l2_branch: "CTyunOS-2.0".to_string(),
            l2_repo_path: "/opt/repo/glibc".to_string(),
            tracking_status: "active".to_string(),
            last_sync_time: None,
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: chrono::Utc::now(),
            last_error: None,
            updated_at: chrono::Utc::now(),
        }
    }

    fn create_mock_package(id: i32) -> packages::Model {
        packages::Model {
            id,
            name: "glibc".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: Some("https://github.com/bminor/glibc".to_string()),
            description: Some("GNU C Library".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_tracking_list_query_defaults() {
        let query = TrackingListQuery {
            page: None,
            page_size: None,
            package_id: None,
            distro_id: None,
            tracking_status: None,
        };
        assert!(query.page.is_none());
        assert!(query.package_id.is_none());
    }

    #[test]
    fn test_create_tracking_request_validation() {
        let req = CreateTrackingRequest {
            package_id: 1,
            distro_id: 1,
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l1_branch: "main".to_string(),
            l2_branch: "main".to_string(),
            l2_repo_path: "/path/to/repo".to_string(),
            tracking_status: Some("active".to_string()),
        };
        assert_eq!(req.package_id, 1);
        assert_eq!(req.l1_repo_owner, "owner");
    }

    #[tokio::test]
    async fn test_list_tracking_invalid_page() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);

        let query = TrackingListQuery {
            page: Some(0), // Invalid: must be >= 1
            page_size: None,
            package_id: None,
            distro_id: None,
            tracking_status: None,
        };

        let result = list_tracking(State(state), Query(query)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::BadRequest(msg) => assert!(msg.contains("Page must be >= 1")),
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[tokio::test]
    async fn test_list_tracking_invalid_page_size() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);

        let query = TrackingListQuery {
            page: Some(1),
            page_size: Some(101), // Invalid: must be <= 100
            package_id: None,
            distro_id: None,
            tracking_status: None,
        };

        let result = list_tracking(State(state), Query(query)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::BadRequest(msg) => assert!(msg.contains("Page size must be between")),
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[tokio::test]
    async fn test_create_tracking_invalid_package_id() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);

        let req = CreateTrackingRequest {
            package_id: 0, // Invalid
            distro_id: 1,
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l1_branch: "main".to_string(),
            l2_branch: "main".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: None,
        };

        let result = create_tracking(State(state), Json(req)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::BadRequest(msg) => assert!(msg.contains("Invalid package_id")),
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[tokio::test]
    async fn test_create_tracking_empty_owner() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);

        let req = CreateTrackingRequest {
            package_id: 1,
            distro_id: 1,
            l1_repo_owner: "".to_string(), // Empty
            l1_repo_name: "repo".to_string(),
            l1_branch: "main".to_string(),
            l2_branch: "main".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: None,
        };

        let result = create_tracking(State(state), Json(req)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::BadRequest(msg) => assert!(msg.contains("l1_repo_owner is required")),
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[tokio::test]
    async fn test_create_tracking_empty_repo_name() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);

        let req = CreateTrackingRequest {
            package_id: 1,
            distro_id: 1,
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "".to_string(), // Empty
            l1_branch: "main".to_string(),
            l2_branch: "main".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: None,
        };

        let result = create_tracking(State(state), Json(req)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::BadRequest(msg) => assert!(msg.contains("l1_repo_name is required")),
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[tokio::test]
    async fn test_create_tracking_package_not_found() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<packages::Model, _, _>([vec![]]) // Package not found
            .into_connection();
        let state = AppState::without_external_clients(db);

        let req = CreateTrackingRequest {
            package_id: 999,
            distro_id: 1,
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l1_branch: "main".to_string(),
            l2_branch: "main".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: None,
        };

        let result = create_tracking(State(state), Json(req)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::NotFound(msg) => assert!(msg.contains("Package 999 not found")),
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test]
    async fn test_create_tracking_success() {
        let mock_package = create_mock_package(1);
        let mock_tracking = create_mock_tracking(1, 1);

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[mock_package]]) // Package exists
            .append_query_results([[mock_tracking]]) // Created tracking
            .into_connection();
        let state = AppState::without_external_clients(db);

        let req = CreateTrackingRequest {
            package_id: 1,
            distro_id: 1,
            l1_repo_owner: "openeuler".to_string(),
            l1_repo_name: "glibc".to_string(),
            l1_branch: "main".to_string(),
            l2_branch: "CTyunOS-2.0".to_string(),
            l2_repo_path: "/opt/repo/glibc".to_string(),
            tracking_status: Some("active".to_string()),
        };

        let result = create_tracking(State(state), Json(req)).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0.code, 201);
    }
