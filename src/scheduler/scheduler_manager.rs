/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

//! 调度器管理器
//!
//! 负责管理所有同步任务的调度和执行

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

use crate::ecosystem::maintenance::MaintenanceService;
use crate::entities::{prelude::TrackingReports, tracking, tracking_reports};

use super::{
    mail_service::MailService,
    report_artifacts::{CveFixComparisonInput, ReportArtifact, RoundCveFixComparisonWriter},
    MaintenanceSyncService, PipelineExecutor, PipelineStage, SyncApiClient, SyncJobResult,
    SyncManager, SyncResult,
};

/// 调度器配置
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub max_concurrent_jobs: usize,
    pub job_timeout_secs: u64,
    pub cleanup_interval_secs: u64,
    pub health_check_interval_secs: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 10,
            job_timeout_secs: 1800,         // 30 分钟
            cleanup_interval_secs: 3600,    // 1 小时
            health_check_interval_secs: 30, // 30 秒
        }
    }
}

/// 调度器状态
#[derive(Debug, Clone)]
pub struct SchedulerStatus {
    pub running: bool,
    pub active_jobs: usize,
    pub pending_jobs: usize,
    pub total_jobs_executed: usize,
    pub last_execution: Option<chrono::DateTime<Utc>>,
}

/// 唤醒信号
#[derive(Debug, Clone)]
pub enum WakeSignal {
    /// 唤醒所有待处理任务
    All,
    /// 唤醒指定的 tracking_id
    Specific(i32),
}

/// 调度器管理器
pub struct SchedulerManager {
    db: Arc<DatabaseConnection>,
    client: Option<Arc<dyn SyncApiClient>>,
    config: SchedulerConfig,
    status: Arc<RwLock<SchedulerStatus>>,
    /// 用于唤醒调度循环的发送器
    wake_tx: mpsc::UnboundedSender<WakeSignal>,
}

impl SchedulerManager {
    /// 创建新的调度器管理器，返回管理器和接收器
    pub fn new(
        db: Arc<DatabaseConnection>,
        client: Option<Arc<dyn SyncApiClient>>,
        config: SchedulerConfig,
    ) -> (Self, mpsc::UnboundedReceiver<WakeSignal>) {
        let status = SchedulerStatus {
            running: false,
            active_jobs: 0,
            pending_jobs: 0,
            total_jobs_executed: 0,
            last_execution: None,
        };

        let (wake_tx, wake_rx) = mpsc::unbounded_channel();

        let manager = Self {
            db,
            client,
            config,
            status: Arc::new(RwLock::new(status)),
            wake_tx,
        };

        (manager, wake_rx)
    }

    /// 启动调度器
    pub async fn start(&mut self) -> Result<()> {
        info!("启动调度器管理器");

        let mut status = self.status.write().await;
        status.running = true;

        info!(
            max_concurrent_jobs = self.config.max_concurrent_jobs,
            "调度器已启动"
        );

        Ok(())
    }

    /// 停止调度器
    pub async fn stop(&mut self) -> Result<()> {
        info!("停止调度器管理器");

        let mut status = self.status.write().await;
        status.running = false;

        info!("调度器已停止");

        Ok(())
    }

    /// 手动触发同步
    pub async fn trigger_manual_sync(&self, tracking_id: i32) -> Result<i64> {
        info!(tracking_id = tracking_id, "手动触发同步");

        let sync_manager = SyncManager::new(&self.db);
        let tracking = sync_manager
            .get_tracking(tracking_id)
            .await
            .context("获取 tracking 失败")?;
        let package_id = tracking.package_id;

        // 创建 sync_job
        let job = sync_manager
            .queue_sync_job(tracking_id, 0)
            .await
            .context("创建 sync_job 失败")?;

        let job_id = job.id;

        // 执行流水线
        let executor = PipelineExecutor::new(&self.db, self.client.clone());

        match executor.execute_sync_job(job_id).await {
            Ok(result) => {
                info!(
                    job_id = job_id,
                    tracking_id = tracking_id,
                    success = result.success,
                    "手动同步完成"
                );

                // 更新状态
                let mut status = self.status.write().await;
                status.total_jobs_executed += 1;
                status.last_execution = Some(Utc::now());

                if result.success {
                    let mut writer = RoundCveFixComparisonWriter::new();
                    let mut report_ids = Vec::new();
                    match self
                        .append_result_to_round_cve_fix_comparison_artifact(
                            &result,
                            &mut writer,
                            &mut report_ids,
                        )
                        .await
                    {
                        Ok(Some(_artifact)) => {
                            for artifact in writer.artifacts() {
                                self.send_cve_fix_comparison_artifact_if_enabled(artifact)
                                    .await;
                            }
                        }
                        Ok(None) => {}
                        Err(err) => {
                            warn!(
                                tracking_id = tracking_id,
                                error = %err,
                                "手动同步生成 CVE 漏洞修复对比 xlsx 失败，但不影响同步结果"
                            );
                        }
                    }
                }

                if result.success {
                    if let Err(err) = MaintenanceService::new(&self.db)
                        .refresh_package(package_id)
                        .await
                    {
                        warn!(
                            tracking_id = tracking_id,
                            package_id = package_id,
                            error = %err,
                            "手动同步 xlsx 处理完成后 maintenance 刷新失败，但不影响同步结果"
                        );
                    }
                }

                sync_manager
                    .complete_sync_task_with_result(
                        tracking_id,
                        &sync_result_from_job_result(&result),
                    )
                    .await?;
            }
            Err(err) => {
                error!(
                    job_id = job_id,
                    tracking_id = tracking_id,
                    error = %err,
                    "手动同步失败"
                );

                sync_manager
                    .complete_sync_task_with_result(
                        tracking_id,
                        &failed_sync_result(err.to_string()),
                    )
                    .await?;

                return Err(err);
            }
        }

        Ok(job_id)
    }

    /// 获取调度器状态
    pub async fn get_scheduler_status(&self) -> Result<SchedulerStatus> {
        let status = self.status.read().await;
        Ok(status.clone())
    }

    /// 唤醒调度循环，立即执行调度
    ///
    /// # 参数
    /// * `tracking_id` - 可选的 tracking_id，如果指定则只处理该任务，否则处理所有待处理任务
    pub fn wake(&self, tracking_id: Option<i32>) {
        let signal = match tracking_id {
            Some(id) => {
                info!(tracking_id = id, "手动唤醒调度器（指定任务）");
                WakeSignal::Specific(id)
            }
            None => {
                info!("手动唤醒调度器（所有任务）");
                WakeSignal::All
            }
        };

        if let Err(e) = self.wake_tx.send(signal) {
            error!("发送唤醒信号失败: {}", e);
        }
    }

    pub async fn execute_round(&self) -> Result<Vec<SyncJobResult>> {
        self.execute_round_wake_up(false, None).await
    }

    /// 执行一轮调度
    ///
    /// # 参数
    /// * `wake_up` - 是否唤醒调度器
    /// * `tracking_id` - 可选的 tracking_id，如果指定则只处理该任务，否则处理所有待处理任务
    pub async fn execute_round_wake_up(
        &self,
        wake_up: bool,
        tracking_id: Option<i32>,
    ) -> Result<Vec<SyncJobResult>> {
        let sync_manager = SyncManager::new(&self.db);

        let mut results = Vec::new();
        let executor = PipelineExecutor::new(&self.db, self.client.clone());
        let mut round_artifact_writer = RoundCveFixComparisonWriter::new();
        let mut round_artifact_report_ids: Vec<i32> = Vec::new();
        let mut packages_to_refresh_after_artifacts: Vec<(i32, i32)> = Vec::new();

        let mut round = 0;
        let wake_flag = wake_up;

        info!("get_pending_sync_tasks_with_tracking_id,wake_up={wake_up}",);
        let mut pending_tasks = sync_manager
            .get_pending_sync_tasks_with_tracking_id(wake_flag, tracking_id)
            .await
            .context("获取待处理任务失败")?;
        if tracking_id.is_none() {
            let before_count = pending_tasks.len();
            pending_tasks.retain(|track| !is_l2_newer_fallback_tracking(track));
            let skipped_count = before_count.saturating_sub(pending_tasks.len());
            if skipped_count > 0 {
                info!(
                    skipped_count,
                    "跳过 openEuler-24.09 fallback tracking 常规调度，仅在 L2Newer 时按需使用"
                );
            }
        }
        order_pending_tasks_for_l2_newer_fallback(&mut pending_tasks);
        loop {
            let pending_count = pending_tasks.len();
            info!(round = round + 1, pending_count, "发现待处理任务");

            {
                let mut status = self.status.write().await;
                status.pending_jobs = pending_count;
            }

            if pending_count == 0 {
                break;
            }
            let limit = self.config.max_concurrent_jobs.min(pending_count);
            let mut executed_in_round = 0;

            let to_process: Vec<_> = pending_tasks.drain(0..limit).collect();
            for tracking in to_process {
                let tracking_id = tracking.id;

                let job = match sync_manager.queue_sync_job(tracking_id, 0).await {
                    Ok(job) => job,
                    Err(err) => {
                        error!(
                            tracking_id = tracking_id,
                            error = %err,
                            "创建 sync_job 失败"
                        );
                        continue;
                    }
                };

                executed_in_round += 1;

                match executor.execute_sync_job(job.id).await {
                    Ok(result) => {
                        info!(
                            job_id = job.id,
                            tracking_id = tracking_id,
                            success = result.success,
                            "同步任务完成"
                        );

                        let _ = sync_manager
                            .complete_sync_task_with_result(
                                tracking_id,
                                &sync_result_from_job_result(&result),
                            )
                            .await;
                        if result.success {
                            if let Err(err) = self
                                .append_result_to_round_cve_fix_comparison_artifact(
                                    &result,
                                    &mut round_artifact_writer,
                                    &mut round_artifact_report_ids,
                                )
                                .await
                            {
                                warn!(
                                    tracking_id = tracking_id,
                                    error = %err,
                                    "追加调度轮次 CVE 漏洞修复对比 xlsx 失败，但不影响同步任务"
                                );
                            }
                            packages_to_refresh_after_artifacts
                                .push((tracking_id, tracking.package_id));
                        }
                        results.push(result);
                    }
                    Err(err) => {
                        error!(
                            job_id = job.id,
                            tracking_id = tracking_id,
                            error = %err,
                            "同步任务失败"
                        );

                        let _ = sync_manager
                            .complete_sync_task_with_result(
                                tracking_id,
                                &failed_sync_result(err.to_string()),
                            )
                            .await;
                    }
                }
            }

            if executed_in_round == 0 {
                error!("本轮未执行任何任务，停止继续调度");
                break;
            }

            round += 1;
        }

        // 更新状态
        {
            let mut status = self.status.write().await;
            status.total_jobs_executed += results.len();
            status.last_execution = Some(Utc::now());
        }

        for artifact in round_artifact_writer.artifacts() {
            self.send_cve_fix_comparison_artifact_if_enabled(artifact)
                .await;
        }

        for (tracking_id, package_id) in packages_to_refresh_after_artifacts {
            if let Err(err) = MaintenanceService::new(&self.db)
                .refresh_package(package_id)
                .await
            {
                warn!(
                    tracking_id = tracking_id,
                    package_id = package_id,
                    error = %err,
                    "xlsx 和邮件处理完成后 tracking maintenance 刷新失败，但不影响同步任务"
                );
            }
        }

        match MaintenanceSyncService::new(&self.db)
            .refresh_due_packages()
            .await
        {
            Ok(summary) => {
                info!(
                    scanned_packages = summary.scanned_packages,
                    due_packages = summary.due_packages,
                    refreshed_packages = summary.refreshed_packages,
                    failed_packages = summary.failed_packages,
                    skipped_no_repo = summary.skipped_no_repo,
                    skipped_not_due = summary.skipped_not_due,
                    "xlsx 和邮件处理完成后 maintenance 周期刷新完成"
                );
            }
            Err(err) => {
                warn!(error = %err, "xlsx 和邮件处理完成后 maintenance 周期刷新失败，但不影响同步调度轮次");
            }
        }

        info!(executed = results.len(), "调度轮次完成");

        Ok(results)
    }

    async fn append_result_to_round_cve_fix_comparison_artifact(
        &self,
        result: &SyncJobResult,
        writer: &mut RoundCveFixComparisonWriter,
        report_ids: &mut Vec<i32>,
    ) -> Result<Option<ReportArtifact>> {
        let Some(report_stage) = result.stage_results.get(&PipelineStage::ReportGeneration) else {
            return Ok(None);
        };
        let Some(input_value) = report_stage.details.get("cve_fix_comparison_input") else {
            return Ok(None);
        };
        let input: CveFixComparisonInput = serde_json::from_value(input_value.clone())
            .with_context(|| {
                format!("解析 tracking {} 的 xlsx 汇总输入失败", result.tracking_id)
            })?;
        if input.commit_reports.is_empty() {
            return Ok(None);
        }

        let report_id = report_stage
            .details
            .get("report_id")
            .and_then(|v| v.as_i64())
            .map(|id| id as i32);
        let artifact = writer.append_input(input)?;
        let artifact_path = artifact.path.clone();
        if let Some(report_id) = report_id {
            if !report_ids.contains(&report_id) {
                report_ids.push(report_id);
            }
        }

        for report_id in report_ids.iter().copied() {
            self.update_report_with_round_cve_fix_comparison_artifact(report_id, &artifact)
                .await?;
        }

        info!(
            path = %artifact_path,
            tracking_id = result.tracking_id,
            rows = artifact.rows,
            "追加调度轮次 CVE 漏洞修复对比 xlsx 成功"
        );
        if artifact.rows == 0 {
            warn!(
                path = %artifact_path,
                tracking_id = result.tracking_id,
                "CVE 漏洞修复对比 xlsx 当前无数据行，请检查系统版本黑名单或 commit_reports 是否为空"
            );
        }

        Ok(Some(artifact))
    }

    async fn send_cve_fix_comparison_artifact_if_enabled(&self, artifact: &ReportArtifact) {
        let mail_service = MailService::from_env();
        if !mail_service.enabled() {
            return;
        }

        if let Err(err) = mail_service.send_xlsx_artifact(artifact).await {
            warn!(
                path = %artifact.path,
                error = %err,
                "发送 CVE 漏洞修复对比 xlsx 邮件失败，但不影响调度结果"
            );
        }
    }

    async fn update_report_with_round_cve_fix_comparison_artifact(
        &self,
        report_id: i32,
        artifact: &ReportArtifact,
    ) -> Result<()> {
        let Some(report) = TrackingReports::find_by_id(report_id)
            .one(&*self.db)
            .await?
        else {
            return Ok(());
        };
        let artifact_value = serde_json::to_value(artifact)?;
        let mut diff_summary = report.diff_summary.clone();
        if let Some(object) = diff_summary.as_object_mut() {
            object.insert(
                "artifacts".to_string(),
                serde_json::json!({
                    "cve_fix_comparison_xlsx": artifact_value,
                }),
            );
        }

        let mut active: tracking_reports::ActiveModel = report.into();
        active.diff_summary = Set(diff_summary);
        active.updated_at = Set(Utc::now());
        active.update(&*self.db).await?;

        Ok(())
    }
}

fn order_pending_tasks_for_l2_newer_fallback(tasks: &mut [tracking::Model]) {
    tasks.sort_by(|left, right| {
        fallback_order_key(left)
            .cmp(&fallback_order_key(right))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn is_l2_newer_fallback_tracking(track: &tracking::Model) -> bool {
    let is_target_l2 = ["25.05", "25.07"]
        .iter()
        .any(|branch| track.l2_branch.contains(branch));
    is_target_l2 && track.l1_branch == "openEuler-24.09"
}

fn fallback_order_key(track: &tracking::Model) -> (i32, String, i32) {
    let is_target_l2 = ["25.05", "25.07"]
        .iter()
        .any(|branch| track.l2_branch.contains(branch));
    let l1_priority = if is_target_l2 && track.l1_branch == "openEuler-24.03-LTS-SP3" {
        0
    } else if is_target_l2 && track.l1_branch == "openEuler-24.09" {
        1
    } else {
        2
    };
    (track.package_id, track.l2_branch.clone(), l1_priority)
}

fn sync_result_from_job_result(result: &SyncJobResult) -> SyncResult {
    if result.success {
        SyncResult::success(0, 0)
    } else {
        failed_sync_result(result.message.clone())
    }
}

fn failed_sync_result(message: impl Into<String>) -> SyncResult {
    SyncResult {
        status: super::SyncStatus::Failed,
        commits_synced: 0,
        issues_synced: 0,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests_basic {
    use super::*;
    use sea_orm::{DatabaseBackend, MockDatabase};

    fn test_tracking_model(id: i32, l1_branch: &str, l2_branch: &str) -> tracking::Model {
        tracking::Model {
            id,
            package_id: 1,
            distro_id: 1,
            l1_branch: l1_branch.to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: l2_branch.to_string(),
            l2_repo_path: "/tmp/l2".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
            platform: Some("Gitee".to_string()),
        }
    }

    #[tokio::test]
    async fn test_scheduler_start_stop_status() {
        let db = Arc::new(MockDatabase::new(DatabaseBackend::Postgres).into_connection());
        let config = SchedulerConfig::default();
        let (mut manager, _wake_rx) = SchedulerManager::new(db, None, config);

        manager.start().await.unwrap();
        let status = manager.get_scheduler_status().await.unwrap();
        assert!(status.running);

        manager.stop().await.unwrap();
        let status = manager.get_scheduler_status().await.unwrap();
        assert!(!status.running);
    }

    #[tokio::test]
    async fn test_wake_signals() {
        let db = Arc::new(MockDatabase::new(DatabaseBackend::Postgres).into_connection());
        let config = SchedulerConfig::default();
        let (manager, mut wake_rx) = SchedulerManager::new(db, None, config);

        manager.wake(None);
        let msg = wake_rx.recv().await.unwrap();
        match msg {
            WakeSignal::All => {}
            _ => panic!("unexpected wake signal"),
        }

        manager.wake(Some(5));
        let msg = wake_rx.recv().await.unwrap();
        match msg {
            WakeSignal::Specific(id) => assert_eq!(id, 5),
            _ => panic!("unexpected wake signal"),
        }
    }

    #[tokio::test]
    async fn test_execute_round_empty() {
        use crate::entities::{packages, tracking};

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<(tracking::Model, Option<packages::Model>), _, _>(vec![vec![]])
            .into_connection();
        let db = Arc::new(db);

        let config = SchedulerConfig::default();
        let (manager, _wake_rx) = SchedulerManager::new(db, None, config);
        let results = manager.execute_round().await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_l2_newer_fallback_tracking_is_filtered_from_regular_round() {
        let primary = test_tracking_model(1, "openEuler-24.03-LTS-SP3", "CTyunOS25.07");
        let fallback = test_tracking_model(2, "openEuler-24.09", "CTyunOS25.07");
        let regular = test_tracking_model(3, "openEuler-24.09", "CTyunOS23.01");

        assert!(!is_l2_newer_fallback_tracking(&primary));
        assert!(is_l2_newer_fallback_tracking(&fallback));
        assert!(!is_l2_newer_fallback_tracking(&regular));

        let mut tasks = vec![fallback, regular.clone(), primary.clone()];
        tasks.retain(|track| !is_l2_newer_fallback_tracking(track));
        order_pending_tasks_for_l2_newer_fallback(&mut tasks);

        assert_eq!(tasks.len(), 2);
        assert!(tasks.iter().any(|task| task.id == primary.id));
        assert!(tasks.iter().any(|task| task.id == regular.id));
        assert!(!tasks.iter().any(|task| task.id == 2));
    }
}

#[cfg(test)]
mod tests_extra {
    use super::*;
    use sea_orm::{DatabaseBackend, MockDatabase};

    #[tokio::test]
    async fn test_scheduler_manager_lifecycle() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let db = Arc::new(db);
        let config = SchedulerConfig::default();

        let (mut manager, _rx) = SchedulerManager::new(db, None, config);

        assert!(!manager.get_scheduler_status().await.unwrap().running);

        manager.start().await.unwrap();
        assert!(manager.get_scheduler_status().await.unwrap().running);

        manager.stop().await.unwrap();
        assert!(!manager.get_scheduler_status().await.unwrap().running);
    }

    #[tokio::test]
    async fn test_wake() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let db = Arc::new(db);
        let config = SchedulerConfig::default();

        let (manager, mut rx) = SchedulerManager::new(db, None, config);

        manager.wake(Some(123));

        if let Some(signal) = rx.recv().await {
            match signal {
                WakeSignal::Specific(id) => assert_eq!(id, 123),
                _ => panic!("Expected WakeSignal::Specific"),
            }
        } else {
            panic!("Expected wake signal");
        }

        manager.wake(None);
        if let Some(signal) = rx.recv().await {
            match signal {
                WakeSignal::All => (),
                _ => panic!("Expected WakeSignal::All"),
            }
        } else {
            panic!("Expected wake signal");
        }
    }

    #[tokio::test]
    async fn test_trigger_manual_sync_skipped_l1() {
        use crate::entities::{sync_jobs, tracking};
        use chrono::Utc;

        let job = sync_jobs::Model {
            id: 77,
            tracking_id: 200,
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
            id: 200,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/tmp/l2".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            // trigger_manual_sync: get_tracking
            .append_query_results::<tracking::Model, _, _>(vec![vec![track.clone()]])
            // queue_sync_job: find_active_sync_job
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![]])
            // queue_sync_job: find_retryable_failed_job
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![]])
            // queue_sync_job: Tracking::find_by_id
            .append_query_results::<tracking::Model, _, _>(vec![vec![track.clone()]])
            // queue_sync_job: insert sync_job
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            .append_query_results::<tracking::Model, _, _>(vec![vec![track.clone()]])
            .append_query_results::<tracking::Model, _, _>(vec![vec![track.clone()]])
            .append_query_results::<tracking::Model, _, _>(vec![vec![track.clone()]])
            .append_query_results::<tracking::Model, _, _>(vec![vec![track.clone()]])
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            .into_connection();

        let db = Arc::new(db);
        let config = SchedulerConfig::default();
        let (manager, _rx) = SchedulerManager::new(db, None, config);

        let job_id = manager.trigger_manual_sync(200).await.unwrap();
        assert_eq!(job_id, 77);

        let status = manager.get_scheduler_status().await.unwrap();
        assert_eq!(status.total_jobs_executed, 1);
        assert!(status.last_execution.is_some());
    }

    #[tokio::test]
    async fn test_execute_round_wake_specific_single() {
        use crate::entities::{compare_reports, packages, sync_jobs, tracking, tracking_reports};
        use chrono::Utc;
        use sea_orm::{DatabaseBackend, MockDatabase};

        let package_model = packages::Model {
            id: 1,
            name: "pkg".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: None,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let track = tracking::Model {
            id: 300,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/tmp/l2".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let job = sync_jobs::Model {
            id: 88,
            tracking_id: 300,
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

        let now = Utc::now();
        let compare_model = compare_reports::Model {
            id: 42,
            tracking_id: track.id,
            generated_at: now,
            l2_vs_l1_diff: None,
            l1_vs_l0_diff: None,
            status: "success".to_string(),
            failure_reason: None,
            created_at: now,
            updated_at: now,
        };

        let tracking_report = tracking_reports::Model {
            id: 1,
            tracking_id: track.id,
            generated_at: now,
            diff_summary: serde_json::json!({}),
            representative_changes: None,
            source: "pipeline".to_string(),
            status: "success".to_string(),
            failure_reason: None,
            created_at: now,
            updated_at: now,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<(tracking::Model, Option<packages::Model>), _, _>(vec![vec![(
                track.clone(),
                Some(package_model.clone()),
            )]])
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            .append_query_results::<tracking::Model, _, _>(vec![vec![track.clone()]])
            .append_query_results::<tracking::Model, _, _>(vec![vec![track.clone()]])
            .append_query_results::<tracking::Model, _, _>(vec![vec![track.clone()]])
            .append_query_results::<tracking::Model, _, _>(vec![vec![track.clone()]])
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            .append_query_results::<compare_reports::Model, _, _>(vec![vec![compare_model.clone()]])
            .append_query_results::<tracking_reports::Model, _, _>(vec![vec![
                tracking_report.clone()
            ]])
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            .into_connection();

        let db = Arc::new(db);
        let config = SchedulerConfig::default();
        let (manager, _rx) = SchedulerManager::new(db, None, config);

        let results = manager
            .execute_round_wake_up(true, Some(300))
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            !results[0].success,
            "当前测试未 mock 完整流水线依赖（如 L1/L2 快照与对比数据），因此应失败"
        );
    }

    #[tokio::test]
    async fn test_trigger_manual_sync_error_job_missing() {
        use crate::entities::{sync_jobs, tracking};
        use chrono::Utc;
        use sea_orm::{DatabaseBackend, MockDatabase};

        let job = sync_jobs::Model {
            id: 99,
            tracking_id: 400,
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
            id: 400,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/tmp/l2".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            // queue_sync_job: find_active_sync_job
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            // execute_sync_job: get_sync_job -> empty (error path)
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![]])
            // apply_completion: Tracking::find_by_id
            .append_query_results::<tracking::Model, _, _>(vec![vec![track.clone()]])
            // apply_completion: update tracking (RETURNING 模拟)
            .append_query_results::<tracking::Model, _, _>(vec![vec![track.clone()]])
            // apply_completion: find_latest_sync_job
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            // apply_completion: update job (RETURNING 模拟)
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job.clone()]])
            .into_connection();

        let db = Arc::new(db);
        let config = SchedulerConfig::default();
        let (manager, _rx) = SchedulerManager::new(db, None, config);

        let result = manager.trigger_manual_sync(400).await;
        assert!(result.is_err());
    }
}
