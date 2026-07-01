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

//! 报告查询路由

use axum::{routing::get, Router};

use crate::server::{handlers::reports, state::AppState};

/// 创建报告查询路由
pub fn reports_routes() -> Router<AppState> {
    Router::new()
        .route("/reports", get(reports::list_reports))
        .route("/reports/:id", get(reports::get_report))
        .route("/reports/:id/export", get(reports::export_report))
}
