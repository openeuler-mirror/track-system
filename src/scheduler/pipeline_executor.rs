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

//! 流水线执行器
//!
//! 负责执行完整的同步流水线：L1获取 → L2快照 → 差异对比 → 分类 → 报告 → 回合建议

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::entities::{sync_jobs, tracking};
use crate::telemetry::Telemetry;

use super::{SyncApiClient, SyncManager};

/// 流水线阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PipelineStage {
    L1Ingestion,
    L2Snapshot,
    DiffComparison,
    Classification,
    ReportGeneration,
    BackportSuggestion,
}

impl PipelineStage {
    /// 获取阶段名称
    pub fn name(&self) -> &'static str {
        match self {
            Self::L1Ingestion => "L1 元数据获取",
            Self::L2Snapshot => "L2 快照生成",
            Self::DiffComparison => "差异对比",
            Self::Classification => "变更分类",
            Self::ReportGeneration => "报告生成",
            Self::BackportSuggestion => "回合建议",
        }
    }

    /// 获取所有阶段（按执行顺序）
    pub fn all_stages() -> Vec<Self> {
        vec![
            Self::L1Ingestion,
            Self::L2Snapshot,
            Self::DiffComparison,
            Self::Classification,
            Self::ReportGeneration,
            Self::BackportSuggestion,
        ]
    }
}

/// 阶段执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResult {
    pub stage: PipelineStage,
    pub success: bool,
    pub message: String,
    pub duration: Duration,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub details: serde_json::Value,
}

impl StageResult {
    pub fn success(
        stage: PipelineStage,
        message: String,
        started_at: DateTime<Utc>,
        details: serde_json::Value,
    ) -> Self {
        let finished_at = Utc::now();
        let duration = (finished_at - started_at)
            .to_std()
            .unwrap_or(Duration::from_secs(0));

        Self {
            stage,
            success: true,
            message,
            duration,
            started_at,
            finished_at,
            details,
        }
    }

    pub fn failure(stage: PipelineStage, message: String, started_at: DateTime<Utc>) -> Self {
        let finished_at = Utc::now();
        let duration = (finished_at - started_at)
            .to_std()
            .unwrap_or(Duration::from_secs(0));

        Self {
            stage,
            success: false,
            message,
            duration,
            started_at,
            finished_at,
            details: serde_json::json!({}),
        }
    }
}

/// 同步任务执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncJobResult {
    pub job_id: i64,
    pub tracking_id: i32,
    pub success: bool,
    pub message: String,
    pub stage_results: HashMap<PipelineStage, StageResult>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub total_duration: Duration,
}

/// 任务进度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    pub job_id: i64,
    pub tracking_id: i32,
    pub current_stage: Option<PipelineStage>,
    pub completed_stages: Vec<PipelineStage>,
    pub progress_percent: f32,
    pub status: String,
}

/// L1 获取结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1IngestionResult {
    pub commits_synced: usize,
    pub issues_synced: usize,
    pub has_new_data: bool,
