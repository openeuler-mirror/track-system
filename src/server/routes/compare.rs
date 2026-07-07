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

//! 对比分析路由

use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::server::{handlers::compare, state::AppState};

/// 创建对比分析路由
pub fn compare_routes() -> Router<AppState> {
    Router::new()
        // 创建对比任务
        .route("/compare/l1-vs-l0", post(compare::compare_l1_vs_l0))
        .route("/compare/l2-vs-l1", post(compare::compare_l2_vs_l1))
        // 查询和管理对比任务（RESTful 风格）
        .route("/compare/tasks/:id", get(compare::get_compare_status))
        .route("/compare/tasks/:id", delete(compare::cancel_compare_task))
}
