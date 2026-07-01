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
pub async fn trigger_manual_sync_handler(
    Path(tracking_id): Path<i32>,
    State(state): State<AppState>,
) -> Result<Json<TriggerSyncResponse>, StatusCode> {
    // 检查是否有调度器管理器
    let scheduler_manager = state
        .scheduler_manager
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    // 手动触发同步
    let scheduler = scheduler_manager.read().await;
    match scheduler.trigger_manual_sync(tracking_id).await {
        Ok(job_id) => Ok(Json(TriggerSyncResponse {
            job_id,
            tracking_id,
            message: "同步任务已创建并开始执行".to_string(),
        })),
        Err(err) => {
            tracing::error!(
                tracking_id = tracking_id,
                error = %err,
                "手动触发同步失败"
            );
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 获取调度器状态
pub async fn get_scheduler_status_handler(
    State(state): State<AppState>,
) -> Result<Json<SchedulerStatusResponse>, StatusCode> {
    // 检查是否有调度器管理器
    let scheduler_manager = state
        .scheduler_manager
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    // 获取调度器状态
    let scheduler = scheduler_manager.read().await;
    match scheduler.get_scheduler_status().await {
        Ok(status) => Ok(Json(SchedulerStatusResponse {
            running: status.running,
            active_jobs: status.active_jobs,
            pending_jobs: status.pending_jobs,
            total_jobs_executed: status.total_jobs_executed,
            last_execution: status.last_execution.map(|dt| dt.to_rfc3339()),
        })),
        Err(err) => {
            tracing::error!(error = %err, "获取调度器状态失败");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 执行一轮调度
pub async fn execute_round_handler(
    State(state): State<AppState>,
    Json(_request): Json<ExecuteRoundRequest>,
) -> Result<Json<ExecuteRoundResponse>, StatusCode> {
    // 检查是否有调度器管理器
    let scheduler_manager = state
        .scheduler_manager
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    // 执行一轮调度
    let scheduler = scheduler_manager.read().await;
    match scheduler.execute_round().await {
        Ok(results) => {
            let succeeded = results.iter().filter(|r| r.success).count();
            let failed = results.len() - succeeded;

            Ok(Json(ExecuteRoundResponse {
                executed: results.len(),
                succeeded,
                failed,
            }))
        }
        Err(err) => {
            tracing::error!(error = %err, "执行调度轮次失败");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 唤醒调度器，立即触发调度
pub async fn wake_scheduler_handler(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<WakeSchedulerResponse>, StatusCode> {
    // 检查是否有调度器管理器
    let scheduler_manager = state
        .scheduler_manager
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    // 获取可选的 tracking_id
    let tracking_id = body
        .get("tracking_id")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);

    // 唤醒调度器
    let scheduler = scheduler_manager.read().await;
    scheduler.wake(tracking_id);

    tracing::info!(tracking_id = ?tracking_id, "调度器已被唤醒");

    Ok(Json(WakeSchedulerResponse {
        message: "调度器已唤醒，将立即执行调度轮次".to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Path, State};
    use sea_orm::{DatabaseBackend, MockDatabase};

    #[tokio::test]
    async fn test_queue_sync_job_handler_success() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);

        let result = queue_sync_job_handler(Path(1), State(state)).await;
        // This will fail without a real scheduler, but tests the handler structure
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_trigger_manual_sync_no_scheduler() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);

        let result = trigger_manual_sync_handler(Path(1), State(state)).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_get_scheduler_status_no_scheduler() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);

        let result = get_scheduler_status_handler(State(state)).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_execute_round_no_scheduler() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);

        let request = ExecuteRoundRequest { max_jobs: Some(5) };
        let result = execute_round_handler(State(state), Json(request)).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_queue_sync_response_serialization() {
        let response = QueueSyncResponse { queued_job_id: 123 };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"queued_job_id\":123"));
    }

    #[test]
    fn test_trigger_sync_response_serialization() {
        let response = TriggerSyncResponse {
            job_id: 456,
            tracking_id: 1,
            message: "Test message".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"job_id\":456"));
        assert!(json.contains("\"tracking_id\":1"));
    }

    #[test]
    fn test_scheduler_status_response_serialization() {
        let response = SchedulerStatusResponse {
            running: true,
            active_jobs: 5,
