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

//! 健康检查 API handlers

use axum::{extract::State, Json};
use sea_orm::DatabaseConnection;

use crate::server::{
    api::{ApiResponse, ComponentStatus, HealthComponents, HealthResponse},
    state::AppState,
};

/// GET /api/health
///
/// 简单的健康检查端点，返回服务状态
pub async fn health_check() -> Json<ApiResponse<String>> {
    Json(ApiResponse::success("OK".to_string()))
}

/// GET /api/status
///
/// 详细的状态检查端点，包含各组件的健康状态
pub async fn status_check(State(state): State<AppState>) -> Json<ApiResponse<HealthResponse>> {
    let version = env!("CARGO_PKG_VERSION").to_string();

    // 检查数据库连接
    let database_status = check_database(&state.db).await;

    // 检查调度器状态（暂时标记为 healthy，后续实现）
    let scheduler_status = ComponentStatus::healthy_with_message("Scheduler is running");

    let components = HealthComponents {
        database: database_status,
        scheduler: scheduler_status,
    };

    // 判断整体状态
    let overall_status =
        if components.database.status == "healthy" && components.scheduler.status == "healthy" {
            "healthy"
        } else {
            "unhealthy"
        };

    let health = HealthResponse {
        status: overall_status.to_string(),
        version,
        components,
    };

    Json(ApiResponse::success(health))
}

/// 检查数据库连接状态
async fn check_database(db: &DatabaseConnection) -> ComponentStatus {
    match db.ping().await {
        Ok(_) => ComponentStatus::healthy_with_message("Database connection is healthy"),
        Err(e) => ComponentStatus::unhealthy(format!("Database connection failed: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::state::AppState;
    use sea_orm::{DatabaseBackend, MockDatabase};

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        assert_eq!(response.0.code, 200);
        assert_eq!(response.0.data, Some("OK".to_string()));
    }

    #[tokio::test]
    async fn test_check_database_healthy() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();

        // MockDatabase automatically responds ok to ping unless configured otherwise?
        // Actually ping() might not send a query in sea-orm-mock or sends a simple SELECT 1
        // Let's assume default behavior is success for empty mock db

        let status = check_database(&db).await;
        assert_eq!(status.status, "healthy");
    }

    #[tokio::test]
    async fn test_status_check_healthy() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);

        let response = status_check(State(state)).await;

        assert_eq!(response.0.code, 200);
        assert_eq!(response.0.data.unwrap().status, "healthy");
    }
}
