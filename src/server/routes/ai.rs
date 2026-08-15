/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FITNESS FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

use axum::{routing::post, Router};

use crate::server::{handlers::ai, state::AppState};

pub fn ai_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/ai/ecosystem/reports/:id/analyze",
            post(ai::analyze_ecosystem_report),
        )
        .route(
            "/ai/maintenance/reports/:id/analyze",
            post(ai::analyze_maintenance_report),
        )
}
