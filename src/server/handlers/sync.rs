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
use serde::{Deserialize, Serialize};

use crate::server::state::AppState;

#[derive(Debug, Serialize)]
pub struct QueueSyncResponse {
    pub queued_job_id: i64,
}

#[derive(Debug, Serialize)]
pub struct TriggerSyncResponse {
    pub job_id: i64,
    pub tracking_id: i32,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SchedulerStatusResponse {
    pub running: bool,
    pub active_jobs: usize,
    pub pending_jobs: usize,
    pub total_jobs_executed: usize,
    pub last_execution: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteRoundRequest {
    pub max_jobs: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ExecuteRoundResponse {
    pub executed: usize,
    pub succeeded: usize,
    pub failed: usize,
}

#[derive(Debug, Serialize)]
pub struct WakeSchedulerResponse {
    pub message: String,
}

pub async fn queue_sync_job_handler(
    Path(tracking_id): Path<i32>,
    State(state): State<AppState>,
) -> Result<Json<QueueSyncResponse>, StatusCode> {
    let manager = state.scheduler();

    manager
        .queue_sync_job(tracking_id, 0)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)
        .map(|job| {
            Json(QueueSyncResponse {
                queued_job_id: job.id,
            })
        })
}

/// 手动触发同步
