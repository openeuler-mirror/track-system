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
