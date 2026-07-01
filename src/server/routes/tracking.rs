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

//! 跟踪配置管理路由

use axum::{
    routing::{delete, get, post, put},
    Router,
};

use crate::server::{handlers::tracking, state::AppState};

/// 创建跟踪配置管理路由
pub fn tracking_routes() -> Router<AppState> {
    Router::new()
        .route("/tracking", get(tracking::list_tracking))
        .route("/tracking", post(tracking::create_tracking))
        .route("/tracking/:id", get(tracking::get_tracking))
        .route("/tracking/:id", put(tracking::update_tracking))
        .route("/tracking/:id", delete(tracking::delete_tracking))
}
