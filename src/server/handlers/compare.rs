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

//! 对比分析 API handlers

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::server::{
    api::ApiResponse,
    error::{ApiError, ApiResult},
    state::AppState,
};

/// 对比任务状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CompareStatus {
    /// 等待中
    Pending,
    /// 运行中
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
}

/// L1 vs L0 对比请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareL1VsL0Request {
    /// 跟踪配置 ID
    pub tracking_id: i32,
    /// L0 快照 ID（可选，默认使用最新）
    pub l0_snapshot_id: Option<String>,
    /// L1 快照 ID（可选，默认使用最新）
    pub l1_snapshot_id: Option<String>,
}

/// L2 vs L1 对比请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareL2VsL1Request {
    /// 跟踪配置 ID
    pub tracking_id: i32,
    /// L1 快照 ID（可选，默认使用最新）
    pub l1_snapshot_id: Option<String>,
    /// L2 快照 ID（可选，默认使用最新）
    pub l2_snapshot_id: Option<String>,
}

/// 对比任务响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareTaskResponse {
    /// 任务 ID
    pub task_id: String,
    /// 任务状态
    pub status: CompareStatus,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 对比状态响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareStatusResponse {
    /// 任务 ID
    pub task_id: String,
    /// 任务状态
    pub status: CompareStatus,
    /// 进度（0-100）
    pub progress: u8,
    /// 状态消息
    pub message: Option<String>,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// 完成时间
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 报告 ID（完成后可用）
    pub report_id: Option<i64>,
}

/// POST /api/compare/l1-vs-l0
///
/// 创建 L1 vs L0 对比任务
pub async fn compare_l1_vs_l0(
    State(_state): State<AppState>,
    Json(request): Json<CompareL1VsL0Request>,
) -> ApiResult<Json<ApiResponse<CompareTaskResponse>>> {
    // 验证请求
    if request.tracking_id <= 0 {
        return Err(ApiError::BadRequest("Invalid tracking_id".to_string()));
    }

    // TODO: 实现 L1 vs L0 对比逻辑
    // 1. 验证 tracking_id 存在
    // 2. 获取 L0 和 L1 快照
    // 3. 创建异步对比任务
    // 4. 返回任务 ID

    let task_id = uuid::Uuid::new_v4().to_string();
    let response = CompareTaskResponse {
        task_id,
        status: CompareStatus::Pending,
        created_at: chrono::Utc::now(),
    };

    Ok(Json(ApiResponse::created(response)))
}

/// POST /api/compare/l2-vs-l1
///
/// 创建 L2 vs L1 对比任务
pub async fn compare_l2_vs_l1(
    State(_state): State<AppState>,
    Json(request): Json<CompareL2VsL1Request>,
) -> ApiResult<Json<ApiResponse<CompareTaskResponse>>> {
    // 验证请求
    if request.tracking_id <= 0 {
        return Err(ApiError::BadRequest("Invalid tracking_id".to_string()));
    }

    // TODO: 实现 L2 vs L1 对比逻辑
    // 1. 验证 tracking_id 存在
    // 2. 获取 L1 和 L2 快照
    // 3. 创建异步对比任务
    // 4. 返回任务 ID

    let task_id = uuid::Uuid::new_v4().to_string();
    let response = CompareTaskResponse {
        task_id,
        status: CompareStatus::Pending,
        created_at: chrono::Utc::now(),
    };

    Ok(Json(ApiResponse::created(response)))
}

/// GET /api/compare/tasks/:id
///
/// 查询对比任务状态
pub async fn get_compare_status(
    State(_state): State<AppState>,
    Path(task_id): Path<String>,
) -> ApiResult<Json<ApiResponse<CompareStatusResponse>>> {
    // TODO: 实现任务状态查询逻辑
    // 1. 从任务队列或数据库查询任务状态
    // 2. 返回任务详情

    // 临时实现：返回模拟数据
    let response = CompareStatusResponse {
        task_id: task_id.clone(),
        status: CompareStatus::Completed,
        progress: 100,
        message: Some("Comparison completed successfully".to_string()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        completed_at: Some(chrono::Utc::now()),
        report_id: Some(1),
    };

    Ok(Json(ApiResponse::success(response)))
}

/// DELETE /api/compare/tasks/:id
///
/// 取消对比任务
///
/// 符合 RESTful 规范：使用 DELETE 方法取消/删除任务
pub async fn cancel_compare_task(
    State(_state): State<AppState>,
    Path(task_id): Path<String>,
) -> ApiResult<axum::http::StatusCode> {
    // TODO: 实现任务取消逻辑
    // 1. 查找任务
    // 2. 如果任务正在运行，发送取消信号
    // 3. 更新任务状态为 Cancelled
    // 4. 返回 204 No Content

    tracing::info!(task_id = %task_id, "取消对比任务");

    // 临时实现：直接返回成功
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use sea_orm::{DatabaseBackend, MockDatabase};

    #[test]
    fn test_compare_status_serialization() {
        let status = CompareStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");
    }

    #[test]
    fn test_compare_status_deserialization() {
        let json = "\"completed\"";
        let status: CompareStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, CompareStatus::Completed);
    }

    #[tokio::test]
    async fn test_compare_l1_vs_l0_valid_request() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);

        let request = CompareL1VsL0Request {
            tracking_id: 1,
            l0_snapshot_id: Some("snapshot-123".to_string()),
            l1_snapshot_id: Some("snapshot-456".to_string()),
        };

        let result = compare_l1_vs_l0(State(state), Json(request)).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.0.code, 201);
        assert!(response.0.data.is_some());
        let task_response = response.0.data.unwrap();
        assert_eq!(task_response.status, CompareStatus::Pending);
    }

