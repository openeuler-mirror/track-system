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
    // 在 L1 阶段生成的快照文件路径（如果生成）
    pub snapshot_path: Option<String>,
    // 在 L1 阶段生成的快照校验值（如果生成）
    pub snapshot_checksum: Option<String>,
}

/// L2 快照结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2SnapshotResult {
    pub snapshot_id: Option<i64>,
    pub snapshot_path: Option<String>,
    pub files_count: usize,
    pub has_new_data: bool,
}

/// 差异对比结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffComparisonResult {
    pub report_id: Option<i64>,
    pub files_changed: usize,
    pub has_spec_changes: bool,
}

/// 变更分类结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub classified_count: usize,
    pub cve_count: usize,
    pub needs_review_count: usize,
}

/// 报告生成结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportGenerationResult {
    pub report_id: i64,
    pub report_status: String,
}

/// 回合建议结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackportSuggestionResult {
    pub candidates_count: usize,
    pub l0_commits_checked: usize,
}

/// 流水线执行器
#[allow(dead_code)]
pub struct PipelineExecutor<'a> {
    pub(super) db: &'a DatabaseConnection,
    pub(super) client: Option<Arc<dyn SyncApiClient>>,
    sync_manager: SyncManager<'a>,
    state_manager: Option<Arc<super::pipeline_state::PipelineStateManager>>,
}

impl<'a> PipelineExecutor<'a> {
    /// 创建新的流水线执行器
    pub fn new(db: &'a DatabaseConnection, client: Option<Arc<dyn SyncApiClient>>) -> Self {
        Self {
            db,
            client,
            sync_manager: SyncManager::new(db),
            state_manager: None,
        }
    }

    /// 创建带状态管理的流水线执行器
    pub fn with_state_manager(
        db: &'a DatabaseConnection,
        client: Option<Arc<dyn SyncApiClient>>,
        state_manager: Arc<super::pipeline_state::PipelineStateManager>,
    ) -> Self {
        Self {
            db,
            client,
            sync_manager: SyncManager::new(db),
            state_manager: Some(state_manager),
        }
    }

    /// 执行完整的同步流水线
    pub async fn execute_sync_job(&self, job_id: i64) -> Result<SyncJobResult> {
        let started_at = Utc::now();
        info!(job_id = job_id, "开始执行同步流水线");

        // 获取 sync_job 信息
        let job = self.get_sync_job(job_id).await?;
        let tracking_id = job.tracking_id;

        // 创建流水线状态
        if let Some(state_mgr) = &self.state_manager {
            state_mgr.create_state(job_id, tracking_id)?;
            state_mgr.update_job_status(job_id, "running").await?;
        }

        // 获取 tracking 配置
        let tracking = self
            .sync_manager
            .get_tracking(tracking_id)
            .await
