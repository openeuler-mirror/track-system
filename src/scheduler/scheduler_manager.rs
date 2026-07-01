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
