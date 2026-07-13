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
                if let Some(details) = result.details.as_object() {
                    if let Some(has_new_data) = details.get("has_new_data") {
                        if !has_new_data.as_bool().unwrap_or(true) {
                            info!(job_id = job_id, "L2 快照为空，跳转到报告生成阶段");
                            skip_to_report_generation = true;
                            continue;
                        }
                    }
                }
            }

            if skip_to_report_generation && *stage == PipelineStage::ReportGeneration {
                break;
            }
        }

        let finished_at = Utc::now();
        let total_duration = (finished_at - started_at)
            .to_std()
            .unwrap_or(Duration::from_secs(0));

        let success = last_error.is_none() && !cancelled;
        let message = if cancelled {
            "流水线已被取消".to_string()
        } else {
            last_error.unwrap_or_else(|| "流水线执行成功".to_string())
        };

        // 更新最终状态
        if let Some(state_mgr) = &self.state_manager {
            let final_status = if cancelled {
                "cancelled"
            } else if success {
                "completed"
            } else {
                "failed"
            };
            state_mgr.update_job_status(job_id, final_status).await?;
            // 清理状态（可选，保留一段时间用于查询）
            // state_mgr.cleanup_state(job_id);
        }

        let result = SyncJobResult {
            job_id,
            tracking_id,
            success,
            message: message.clone(),
            stage_results,
            started_at,
            finished_at,
            total_duration,
        };

        info!(
            job_id = job_id,
            tracking_id = tracking_id,
            success = success,
            cancelled = cancelled,
            duration = ?total_duration,
            "同步流水线执行完成"
        );

        Ok(result)
    }

    /// 执行单个阶段
    async fn execute_stage(
        &self,
        stage: PipelineStage,
        tracking: &tracking::Model,
        previous_results: &HashMap<PipelineStage, StageResult>,
    ) -> Result<StageResult> {
        let started_at = Utc::now();

        match stage {
            PipelineStage::L1Ingestion => {
                let result = self.stage_l1_ingestion(tracking).await?;
                let details = serde_json::to_value(&result)?;
                Ok(StageResult::success(
                    stage,
                    format!(
                        "获取 {} 个 commits, {} 个 issues",
                        result.commits_synced, result.issues_synced
                    ),
                    started_at,
                    details,
                ))
            }
            PipelineStage::L2Snapshot => {
                let result = self.stage_l2_snapshot(tracking).await?;
                let details = serde_json::to_value(&result)?;
                Ok(StageResult::success(
                    stage,
                    format!("生成快照，包含 {} 个文件", result.files_count),
                    started_at,
                    details,
                ))
            }
            PipelineStage::DiffComparison => {
                let result = self
                    .stage_diff_comparison(tracking, previous_results)
                    .await?;
                let details = serde_json::to_value(&result)?;
                Ok(StageResult::success(
                    stage,
                    format!("对比完成，{} 个文件变更", result.files_changed),
                    started_at,
                    details,
                ))
            }
            PipelineStage::Classification => {
                let result = self
                    .stage_classification(tracking, previous_results)
                    .await?;
                let details = serde_json::to_value(&result)?;
                Ok(StageResult::success(
                    stage,
                    format!(
                        "分类 {} 个 commits，发现 {} 个 CVE",
                        result.classified_count, result.cve_count
                    ),
                    started_at,
                    details,
                ))
            }
            PipelineStage::ReportGeneration => {
                let result = self
                    .stage_report_generation(tracking, previous_results)
                    .await?;
                let details = serde_json::to_value(&result)?;
                Ok(StageResult::success(
                    stage,
                    format!("生成报告 ID: {}", result.report_id),
                    started_at,
                    details,
                ))
            }
            PipelineStage::BackportSuggestion => {
                let result = self
                    .stage_backport_suggestion(tracking, previous_results)
                    .await?;
                let details = serde_json::to_value(&result)?;
                Ok(StageResult::success(
                    stage,
                    format!("生成 {} 个回合候选", result.candidates_count),
                    started_at,
                    details,
                ))
            }
        }
    }

    /// 获取任务进度
    pub async fn get_job_progress(&self, job_id: i64) -> Result<JobProgress> {
        if let Some(state_mgr) = &self.state_manager {
            // 使用状态管理器获取详细进度
            state_mgr.get_progress(job_id).await
        } else {
            // 回退到基础实现
            let job = self.get_sync_job(job_id).await?;
            Ok(JobProgress {
                job_id,
                tracking_id: job.tracking_id,
                current_stage: None,
                completed_stages: vec![],
                progress_percent: 0.0,
                status: job.status,
            })
        }
    }

    /// 取消任务
    pub async fn cancel_job(&self, job_id: i64) -> Result<()> {
        info!(job_id = job_id, "请求取消流水线任务");

        if let Some(state_mgr) = &self.state_manager {
            // 使用状态管理器请求取消
            state_mgr.request_cancel(job_id)?;
            state_mgr.update_job_status(job_id, "cancelling").await?;
            info!(job_id = job_id, "取消请求已记录");
        } else {
            warn!(job_id = job_id, "状态管理器未启用，无法取消任务");
        }

        Ok(())
    }

    /// 获取 sync_job 记录
    async fn get_sync_job(&self, job_id: i64) -> Result<sync_jobs::Model> {
        use crate::entities::prelude::*;
        use sea_orm::EntityTrait;

        SyncJobs::find_by_id(job_id)
            .one(self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("SyncJob {} 不存在", job_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DatabaseBackend, MockDatabase};

    #[test]
    fn test_pipeline_stage_name() {
        assert_eq!(PipelineStage::L1Ingestion.name(), "L1 元数据获取");
        assert_eq!(PipelineStage::L2Snapshot.name(), "L2 快照生成");
        assert_eq!(PipelineStage::DiffComparison.name(), "差异对比");
        assert_eq!(PipelineStage::Classification.name(), "变更分类");
        assert_eq!(PipelineStage::ReportGeneration.name(), "报告生成");
        assert_eq!(PipelineStage::BackportSuggestion.name(), "回合建议");
    }

    #[test]
    fn test_pipeline_stage_all_stages() {
        let stages = PipelineStage::all_stages();
        assert_eq!(stages.len(), 6);
        assert_eq!(stages[0], PipelineStage::L1Ingestion);
        assert_eq!(stages[1], PipelineStage::L2Snapshot);
        assert_eq!(stages[2], PipelineStage::DiffComparison);
        assert_eq!(stages[3], PipelineStage::Classification);
        assert_eq!(stages[4], PipelineStage::ReportGeneration);
        assert_eq!(stages[5], PipelineStage::BackportSuggestion);
    }

    #[test]
    fn test_stage_result_success() {
        let started_at = Utc::now();
        let details = serde_json::json!({"test": "data"});

        let result = StageResult::success(
            PipelineStage::L1Ingestion,
            "成功消息".to_string(),
            started_at,
            details.clone(),
        );

        assert_eq!(result.stage, PipelineStage::L1Ingestion);
        assert!(result.success);
        assert_eq!(result.message, "成功消息");
        assert_eq!(result.details, details);
        assert!(result.finished_at >= started_at);
    }

    #[test]
    fn test_stage_result_failure() {
        let started_at = Utc::now();

        let result = StageResult::failure(
            PipelineStage::L2Snapshot,
            "失败消息".to_string(),
            started_at,
        );

        assert_eq!(result.stage, PipelineStage::L2Snapshot);
        assert!(!result.success);
        assert_eq!(result.message, "失败消息");
        assert_eq!(result.details, serde_json::json!({}));
        assert!(result.finished_at >= started_at);
    }

    #[test]
    fn test_l1_ingestion_result_serialization() {
        let result = L1IngestionResult {
            commits_synced: 10,
            issues_synced: 5,
            has_new_data: true,
            snapshot_path: Some("/path/to/snapshot".to_string()),
            snapshot_checksum: Some("abc123".to_string()),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["commits_synced"], 10);
        assert_eq!(json["issues_synced"], 5);
        assert_eq!(json["has_new_data"], true);
    }

    #[test]
    fn test_l2_snapshot_result_serialization() {
        let result = L2SnapshotResult {
            snapshot_id: Some(123),
            snapshot_path: Some("/path/to/snapshot".to_string()),
            files_count: 42,
            has_new_data: true,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["snapshot_id"], 123);
        assert_eq!(json["files_count"], 42);
        assert_eq!(json["has_new_data"], true);
    }

    #[test]
    fn test_diff_comparison_result_serialization() {
        let result = DiffComparisonResult {
            report_id: Some(456),
            files_changed: 15,
            has_spec_changes: true,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["report_id"], 456);
        assert_eq!(json["files_changed"], 15);
        assert_eq!(json["has_spec_changes"], true);
    }

    #[test]
    fn test_classification_result_serialization() {
        let result = ClassificationResult {
            classified_count: 20,
            cve_count: 3,
            needs_review_count: 7,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["classified_count"], 20);
        assert_eq!(json["cve_count"], 3);
        assert_eq!(json["needs_review_count"], 7);
    }

    #[test]
    fn test_report_generation_result_serialization() {
        let result = ReportGenerationResult {
            report_id: 789,
            report_status: "completed".to_string(),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["report_id"], 789);
        assert_eq!(json["report_status"], "completed");
    }

    #[test]
    fn test_backport_suggestion_result_serialization() {
        let result = BackportSuggestionResult {
            candidates_count: 12,
            l0_commits_checked: 100,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["candidates_count"], 12);
        assert_eq!(json["l0_commits_checked"], 100);
    }

    #[test]
    fn test_job_progress_structure() {
        let progress = JobProgress {
            job_id: 1,
            tracking_id: 2,
            current_stage: Some(PipelineStage::L1Ingestion),
            completed_stages: vec![],
            progress_percent: 0.0,
            status: "running".to_string(),
        };

        assert_eq!(progress.job_id, 1);
        assert_eq!(progress.tracking_id, 2);
        assert!(progress.current_stage.is_some());
        assert_eq!(progress.status, "running");
    }

    #[test]
    fn test_pipeline_executor_creation() {
        let db = sea_orm::MockDatabase::new(sea_orm::DatabaseBackend::Postgres).into_connection();
        let executor = PipelineExecutor::new(&db, None);
        assert!(executor.state_manager.is_none());
        assert!(executor.client.is_none());
    }

    #[tokio::test]
    async fn test_cancel_job_without_state_manager() {
        let db = sea_orm::MockDatabase::new(sea_orm::DatabaseBackend::Postgres).into_connection();
        let executor = PipelineExecutor::new(&db, None);
        let result = executor.cancel_job(1).await;
        // Should succeed but do nothing
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_sync_job_l1_skips_following_stages() {
        use crate::entities::{packages, sync_jobs, tracking, tracking_reports};

        // Prepare job and tracking
        let job = sync_jobs::Model {
            id: 11,
            tracking_id: 21,
            job_kind: "sync".to_string(),
            scheduled_at: Utc::now(),
            started_at: Some(Utc::now()),
            finished_at: None,
            status: "running".to_string(),
            error: None,
            attempt_count: 0,
            priority: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let track = tracking::Model {
            id: 21,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/tmp/l2".to_string(),
            tracking_status: "paused".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let _pkg = packages::Model {
            id: 1,
            name: "pkg".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: None,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let report = tracking_reports::Model {
            id: 1,
            tracking_id: track.id,
            generated_at: Utc::now(),
            diff_summary: serde_json::json!({}),
            representative_changes: None,
            source: "pipeline".to_string(),
            status: "success".to_string(),
            failure_reason: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            .append_query_results::<tracking::Model, _, _>(vec![vec![track.clone()]])
            .append_query_results::<tracking::Model, _, _>(vec![vec![track.clone()]])
            .append_query_results::<packages::Model, _, _>(vec![vec![_pkg.clone()]])
            .append_query_results::<tracking_reports::Model, _, _>(vec![vec![report]])
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            .into_connection();

        // Use a state manager to avoid None paths
        let db_arc = std::sync::Arc::new(db);
        let state_mgr =
            std::sync::Arc::new(crate::scheduler::PipelineStateManager::new(db_arc.clone()));
        let executor =
            PipelineExecutor::with_state_manager(db_arc.as_ref(), None, state_mgr.clone());

        // Execute
        let result = executor.execute_sync_job(11).await.unwrap();
        assert!(result
            .stage_results
            .contains_key(&PipelineStage::L1Ingestion));
        // L1 阶段无新数据时，仍应继续生成报告（便于周期任务留下运行痕迹）
        assert!(result
            .stage_results
            .contains_key(&PipelineStage::ReportGeneration));
    }

    #[tokio::test]
    async fn test_get_job_progress_with_state_manager() {
        use crate::entities::sync_jobs;
        use sea_orm::{DatabaseBackend, MockDatabase};
        use std::sync::Arc;

        let job_model = sync_jobs::Model {
            id: 1,
            tracking_id: 10,
            job_kind: "sync".to_string(),
            scheduled_at: Utc::now(),
            started_at: Some(Utc::now()),
            finished_at: None,
            status: "running".to_string(),
            error: None,
            attempt_count: 0,
            priority: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
