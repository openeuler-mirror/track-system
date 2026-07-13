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

//! 调度器管理器
//!
//! 负责管理所有同步任务的调度和执行

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info};

use super::{PipelineExecutor, SyncApiClient, SyncJobResult, SyncManager};

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

                // 更新 sync_manager 状态
                if result.success {
                    sync_manager.complete_sync_task(tracking_id, true).await?;
                } else {
                    sync_manager.complete_sync_task(tracking_id, false).await?;
                }
            }
            Err(err) => {
                error!(
                    job_id = job_id,
                    tracking_id = tracking_id,
                    error = %err,
                    "手动同步失败"
                );

                sync_manager.complete_sync_task(tracking_id, false).await?;

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

        // 获取待处理的任务（按优先级排序）
        let pending_tasks = sync_manager
            .get_pending_sync_tasks_with_tracking_id(wake_up, tracking_id)
            .await
            .context("获取待处理任务失败")?;

        info!(pending_count = pending_tasks.len(), "发现待处理任务");

        // 更新状态
        {
            let mut status = self.status.write().await;
            status.pending_jobs = pending_tasks.len();
        }

        let mut results = Vec::new();
        let executor = PipelineExecutor::new(&self.db, self.client.clone());

        // 限制并发数量
        let limit = self.config.max_concurrent_jobs.min(pending_tasks.len());

        for tracking in pending_tasks.into_iter().take(limit) {
            let tracking_id = tracking.id;

            // 创建 sync_job
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

            // 执行流水线
            match executor.execute_sync_job(job.id).await {
                Ok(result) => {
                    info!(
                        job_id = job.id,
                        tracking_id = tracking_id,
                        success = result.success,
                        "同步任务完成"
                    );

                    // 更新 sync_manager 状态
                    if result.success {
                        let _ = sync_manager.complete_sync_task(tracking_id, true).await;
                    } else {
                        let _ = sync_manager.complete_sync_task(tracking_id, false).await;
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

                    let _ = sync_manager.complete_sync_task(tracking_id, false).await;
                }
            }
        }

        // 更新状态
        {
            let mut status = self.status.write().await;
            status.total_jobs_executed += results.len();
            status.last_execution = Some(Utc::now());
        }

        info!(executed = results.len(), "调度轮次完成");

        Ok(results)
    }
}

#[cfg(test)]
mod tests_basic {
    use super::*;
    use sea_orm::{DatabaseBackend, MockDatabase};

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
