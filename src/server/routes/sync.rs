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
    routing::{get, post},
    Router,
};

use crate::server::{handlers::sync, state::AppState};

pub fn sync_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/sync/:tracking_id/queue",
            post(sync::queue_sync_job_handler),
        )
        .route(
            "/sync/:tracking_id/trigger",
            post(sync::trigger_manual_sync_handler),
        )
        .route("/scheduler/status", get(sync::get_scheduler_status_handler))
        .route("/scheduler/execute", post(sync::execute_round_handler))
        .route("/scheduler/wake", post(sync::wake_scheduler_handler))
}
