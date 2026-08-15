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

use axum::{
    extract::{Path, State},
    Json,
};

use crate::{
    l0::{L0RepoCacheService, L0RepoCacheWarmItem, L0RepoCacheWarmSummary},
    server::{
        api::ApiResponse,
        error::{ApiError, ApiResult},
        state::AppState,
    },
};

pub async fn warm_package_cache(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<Json<ApiResponse<L0RepoCacheWarmItem>>> {
    let service = L0RepoCacheService::new(state.db.as_ref());
    let result = service.warm_package(id).await.map_err(|error| {
        let message = error.to_string();
        if message.contains("not found") {
            ApiError::NotFound(message)
        } else if message.contains("missing l0_repo_url") {
            ApiError::BadRequest(message)
        } else {
            ApiError::InternalError(message)
        }
    })?;

    Ok(Json(ApiResponse::success(result)))
}

pub async fn warm_all_package_caches(
    State(state): State<AppState>,
) -> ApiResult<Json<ApiResponse<L0RepoCacheWarmSummary>>> {
    let service = L0RepoCacheService::new(state.db.as_ref());
    let result = service
        .warm_all_packages()
        .await
        .map_err(|error| ApiError::InternalError(error.to_string()))?;

    Ok(Json(ApiResponse::success(result)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DatabaseBackend, MockDatabase};

    #[tokio::test]
    async fn warm_package_cache_returns_not_found() {
        let db = MockDatabase::new(DatabaseBackend::Sqlite)
            .append_query_results::<crate::entities::packages::Model, _, _>([[]])
            .into_connection();
        let state = AppState::without_external_clients(db);

        let result = warm_package_cache(State(state), Path(42)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::NotFound(message) => assert!(message.contains("package 42 not found")),
            other => panic!("unexpected error: {:?}", other),
        }
    }
}
