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
