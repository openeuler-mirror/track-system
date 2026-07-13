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

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use sea_orm::*;

use crate::{
    entities::{packages, prelude::*},
    server::{
        dto::{
            CreatePackageRequest, PackageResponse, PackageWithTrackingResponse, TrackingResponse,
            UpdatePackageRequest,
        },
        error::{ApiError, ApiResult},
        state::AppState,
    },
};

/// 列出所有软件包
pub async fn list_packages(State(state): State<AppState>) -> ApiResult<Json<Vec<PackageResponse>>> {
    let packages = Packages::find().all(state.db.as_ref()).await?;

    let responses: Vec<PackageResponse> = packages.into_iter().map(Into::into).collect();

    Ok(Json(responses))
}

/// 创建软件包
pub async fn create_package(
    State(state): State<AppState>,
    Json(req): Json<CreatePackageRequest>,
) -> ApiResult<(StatusCode, Json<PackageResponse>)> {
    let now = Utc::now();

    if req.sync_interval_hours <= 0 || req.sync_interval_hours > 24 * 365 {
        return Err(ApiError::BadRequest(
            "sync_interval_hours 必须在 1..=8760 小时范围内".to_string(),
        ));
    }

    let package = packages::ActiveModel {
        name: Set(req.name),
        level: Set(req.level),
        sync_interval_hours: Set(req.sync_interval_hours),
        l0_repo_url: Set(req.l0_repo_url),
        description: Set(req.description),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };

    let result = package.insert(state.db.as_ref()).await?;

    Ok((StatusCode::CREATED, Json(result.into())))
}

/// 获取单个软件包
pub async fn get_package(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<Json<PackageResponse>> {
    let package = Packages::find_by_id(id)
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Package with id {} not found", id)))?;

    Ok(Json(package.into()))
}

/// 获取软件包及其跟踪配置
pub async fn get_package_with_tracking(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<Json<PackageWithTrackingResponse>> {
    let package = Packages::find_by_id(id)
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Package with id {} not found", id)))?;

    let tracking_list = package
        .find_related(Tracking)
        .all(state.db.as_ref())
        .await?;

    let tracking_responses: Vec<TrackingResponse> =
        tracking_list.into_iter().map(Into::into).collect();

    Ok(Json(PackageWithTrackingResponse {
        package: package.into(),
        tracking: tracking_responses,
    }))
}

/// 更新软件包
pub async fn update_package(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<UpdatePackageRequest>,
) -> ApiResult<Json<PackageResponse>> {
    let package = Packages::find_by_id(id)
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Package with id {} not found", id)))?;

    let mut package: packages::ActiveModel = package.into();

    if let Some(level) = req.level {
        package.level = Set(level);
    }
    if let Some(sync_interval_hours) = req.sync_interval_hours {
        if sync_interval_hours <= 0 || sync_interval_hours > 24 * 365 {
            return Err(ApiError::BadRequest(
                "sync_interval_hours 必须在 1..=8760 小时范围内".to_string(),
            ));
        }
        package.sync_interval_hours = Set(sync_interval_hours);
    }
    if let Some(l0_repo_url) = req.l0_repo_url {
        package.l0_repo_url = Set(Some(l0_repo_url));
    }
    if let Some(description) = req.description {
        package.description = Set(Some(description));
    }

    package.updated_at = Set(Utc::now());

    let result = package.update(state.db.as_ref()).await?;

    Ok(Json(result.into()))
}

/// 删除软件包
pub async fn delete_package(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<StatusCode> {
    let result = Packages::delete_by_id(id).exec(state.db.as_ref()).await?;

    if result.rows_affected == 0 {
        return Err(ApiError::NotFound(format!(
            "Package with id {} not found",
            id
        )));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::dto::{CreatePackageRequest, UpdatePackageRequest};
    use axum::extract::{Path, State};
    use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult};

    fn create_mock_package(id: i32, name: &str) -> packages::Model {
        packages::Model {
            id,
            name: name.to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: Some("https://github.com/example/repo".to_string()),
            description: Some("Test package".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_list_packages_empty() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<packages::Model, _, _>([vec![]])
            .into_connection();

        let state = AppState::without_external_clients(db);
        let result = list_packages(State(state)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0.len(), 0);
    }

    #[tokio::test]
    async fn test_list_packages_with_data() {
        let mock_packages = vec![
            create_mock_package(1, "glibc"),
            create_mock_package(2, "gcc"),
        ];

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([mock_packages])
            .into_connection();

        let state = AppState::without_external_clients(db);
        let result = list_packages(State(state)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0.len(), 2);
        assert_eq!(response.0[0].name, "glibc");
        assert_eq!(response.0[1].name, "gcc");
    }

    #[tokio::test]
    async fn test_create_package() {
        let mock_package = create_mock_package(1, "new-package");

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[mock_package.clone()]])
            .into_connection();

        let state = AppState::without_external_clients(db);
        let request = CreatePackageRequest {
            name: "new-package".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: Some("https://github.com/example/repo".to_string()),
            description: Some("Test package".to_string()),
        };

        let result = create_package(State(state), Json(request)).await;
        assert!(result.is_ok());

        let (status, response) = result.unwrap();
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(response.0.name, "new-package");
    }

    #[tokio::test]
    async fn test_create_package_invalid_sync_interval() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();

        let state = AppState::without_external_clients(db);
        let request = CreatePackageRequest {
            name: "new-package".to_string(),
            level: 1,
            sync_interval_hours: 0,
            l0_repo_url: Some("https://github.com/example/repo".to_string()),
            description: Some("Test package".to_string()),
        };

        let result = create_package(State(state), Json(request)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));
    }

    #[tokio::test]
    async fn test_get_package_found() {
        let mock_package = create_mock_package(1, "glibc");

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[mock_package.clone()]])
            .into_connection();

        let state = AppState::without_external_clients(db);
        let result = get_package(State(state), Path(1)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0.id, 1);
        assert_eq!(response.0.name, "glibc");
    }

    #[tokio::test]
    async fn test_get_package_not_found() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<packages::Model, _, _>([[]])
            .into_connection();

        let state = AppState::without_external_clients(db);
        let result = get_package(State(state), Path(999)).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ApiError::NotFound(_)));
    }

    #[tokio::test]
