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
            .context("获取 tracking 配置失败")?;

        let mut stage_results = HashMap::new();
        let mut last_error: Option<String> = None;
        let mut cancelled = false;
        let mut skip_to_report_generation = false;

        // 执行各个阶段
        let stages = PipelineStage::all_stages();
        for (index, stage) in stages.iter().enumerate() {
            if skip_to_report_generation && *stage != PipelineStage::ReportGeneration {
                continue;
            }

            // 检查是否已取消
            if let Some(state_mgr) = &self.state_manager {
                if state_mgr.is_cancelled(job_id) {
                    warn!(job_id = job_id, "流水线已被取消");
                    cancelled = true;
                    last_error = Some("流水线已被用户取消".to_string());
                    break;
                }
            }

            info!(
                job_id = job_id,
                tracking_id = tracking_id,
                stage = ?stage,
                progress = format!("{}/{}", index + 1, stages.len()),
                "执行流水线阶段"
            );

            // 更新当前阶段
            if let Some(state_mgr) = &self.state_manager {
                state_mgr.start_stage(job_id, *stage)?;
            }

            let stage_started_at = Utc::now();

            let result = match self.execute_stage(*stage, &tracking, &stage_results).await {
                Ok(result) => {
                    info!(
                        job_id = job_id,
                        stage = ?stage,
                        duration = ?result.duration,
                        "阶段执行成功"
                    );
                    Telemetry::pipeline_stage_completed(tracking_id, stage.name(), true);
                    result
                }
                Err(err) => {
                    error!(
                        job_id = job_id,
                        stage = ?stage,
                        error = %err,
                        "阶段执行失败"
                    );
                    Telemetry::pipeline_stage_completed(tracking_id, stage.name(), false);

                    let error_msg = format!("阶段 {} 失败: {}", stage.name(), err);
                    last_error = Some(error_msg.clone());

                    StageResult::failure(*stage, error_msg, stage_started_at)
                }
            };

            stage_results.insert(*stage, result.clone());

            // 完成阶段
            if result.success {
                if let Some(state_mgr) = &self.state_manager {
                    state_mgr.complete_stage(job_id, *stage)?;
                }
            }

            // 如果阶段失败，停止后续阶段
            if !result.success {
                warn!(
                    job_id = job_id,
                    stage = ?stage,
                    "阶段失败，停止后续阶段执行"
                );
                break;
            }

            // 优化：如果 L1 获取没有新数据，直接生成报告
            if *stage == PipelineStage::L1Ingestion {
                if let Some(details) = result.details.as_object() {
                    if let Some(has_new_data) = details.get("has_new_data") {
                        if !has_new_data.as_bool().unwrap_or(true) {
                            info!(job_id = job_id, "L1 没有新数据，跳转到报告生成阶段");
                            skip_to_report_generation = true;
                            continue;
                        }
                    }
                }
            }

            // 优化：如果 L2 快照为空，直接生成报告
            if *stage == PipelineStage::L2Snapshot {
