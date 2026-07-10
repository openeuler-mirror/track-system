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

//! Cron调度器
//!
//! 负责根据配置的间隔定期执行同步和分类任务

use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::time::Duration;
use tracing::info;

use super::SyncManager;

/// Cron调度器
pub struct CronScheduler<'a> {
    #[allow(dead_code)]
    db: &'a DatabaseConnection,
    sync_manager: SyncManager<'a>,
}

impl<'a> CronScheduler<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self {
            db,
            sync_manager: SyncManager::new(db),
        }
    }

    /// 运行调度循环（定期执行待处理任务）
    pub async fn run_scheduler_loop(&self, interval_secs: u64) -> Result<()> {
        info!("启动调度器循环，间隔: {}秒", interval_secs);

        loop {
            match self.check_and_queue_pending_tasks().await {
                Ok(count) => {
                    if count > 0 {
                        info!("发现 {} 个待同步任务", count);
                    }
                }
                Err(e) => {
                    tracing::error!("检查待处理任务失败: {}", e);
                }
            }

            tokio::time::sleep(Duration::from_secs(interval_secs)).await;
        }
    }

    /// 检查并入队待处理的同步任务
    async fn check_and_queue_pending_tasks(&self) -> Result<usize> {
        let pending_tasks = self
            .sync_manager
            .get_pending_sync_tasks_ordered(false)
            .await?;
        let count = pending_tasks.len();

        for tracking in pending_tasks {
            // 尝试入队，如果已有待处理或运行中的任务则跳过
            let _ = self.sync_manager.queue_sync_job(tracking.id, 0).await;
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DatabaseBackend, MockDatabase};

    #[test]
    fn test_cron_scheduler_creation() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let _scheduler = CronScheduler::new(&db);
    }

    #[tokio::test]
    async fn test_check_and_queue_pending_tasks_empty() {
        use crate::entities::{packages, tracking};

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<(tracking::Model, Option<packages::Model>), _, _>(vec![vec![]])
            .into_connection();

        let scheduler = CronScheduler::new(&db);
        let result = scheduler.check_and_queue_pending_tasks().await.unwrap();
        assert_eq!(result, 0);
    }

    #[tokio::test]
    async fn test_check_and_queue_pending_tasks_with_data() {
        use crate::entities::{packages, tracking};
        use chrono::{Duration, Utc};

        let tracking_model = tracking::Model {
