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

//! 同步任务执行器
//!
//! 负责实际执行同步任务，将API客户端、数据抓取服务和任务管理器连接起来

use std::sync::Arc;

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use tracing::{error, info, warn};

use crate::{
    collectors::{GitClient, IssueClient},
    entities::tracking,
    telemetry::Telemetry,
};

use super::{SyncManager, SyncResult, SyncService, SyncStatus};

const DEFAULT_MAX_TASKS: usize = 4;

#[derive(Debug, Default, Clone)]
pub struct SyncExecutionStats {
    pub discovered: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub errors: Vec<(i32, String)>,
}

impl SyncExecutionStats {
    fn record_outcome(&mut self, tracking_id: i32, outcome: &SyncResult) {
        self.processed += 1;
        match outcome.status {
            SyncStatus::Success => self.succeeded += 1,
            SyncStatus::Skipped => self.skipped += 1,
            SyncStatus::Failed => {
                self.failed += 1;
                self.errors.push((tracking_id, outcome.message.clone()));
            }
        }
    }

    fn record_error(&mut self, tracking_id: i32, message: String) {
        self.processed += 1;
        self.failed += 1;
        self.errors.push((tracking_id, message));
    }
}

pub trait SyncApiClient: GitClient + IssueClient + Send + Sync {}
impl<T> SyncApiClient for T where T: GitClient + IssueClient + Send + Sync {}

/// 同步执行器
#[allow(dead_code)]
pub struct SyncExecutor<'a> {
    db: &'a DatabaseConnection,
    client: Option<Arc<dyn SyncApiClient>>,
    sync_manager: SyncManager<'a>,
}

impl<'a> SyncExecutor<'a> {
    /// 创建新的同步执行器
    pub fn new(db: &'a DatabaseConnection, client: Option<Arc<dyn SyncApiClient>>) -> Self {
        Self {
            db,
            client,
            sync_manager: SyncManager::new(db),
        }
    }

    /// 执行单个tracking的同步任务
    pub async fn execute_sync(&self, tracking_id: i32) -> Result<SyncResult> {
        info!(tracking_id = tracking_id, "开始执行同步任务");

        // 启动同步任务
        self.sync_manager
            .start_sync_task(tracking_id)
            .await
            .context("启动同步任务失败")?;

        // 获取tracking配置
        let tracking = self
            .sync_manager
            .get_tracking(tracking_id)
            .await
            .context("获取tracking配置失败")?;

        // 执行实际的同步操作
        let outcome = match self.do_sync(&tracking).await {
            Ok(result) => result,
            Err(err) => {
                self.sync_manager
                    .complete_sync_task(tracking_id, false)
                    .await
                    .context("完成同步任务失败")?;

                error!(tracking_id = tracking_id, error = %err, "同步任务执行失败");
                Telemetry::sync_job_failed(tracking_id, 0, &err.to_string());
                return Err(err);
            }
        };

        self.sync_manager
            .complete_sync_task_with_result(tracking_id, &outcome)
            .await
            .context("更新同步任务结果失败")?;

        match outcome.status {
            SyncStatus::Success => {
                info!(tracking_id = tracking_id, "同步任务执行成功");
                Telemetry::sync_job_succeeded(tracking_id, 0);
            }
            SyncStatus::Skipped => {
                warn!(
                    tracking_id = tracking_id,
                    reason = %outcome.message,
                    "同步任务被跳过"
                );
            }
            SyncStatus::Failed => {
                error!(
                    tracking_id = tracking_id,
                    reason = %outcome.message,
                    "同步任务标记为失败"
                );
                Telemetry::sync_job_failed(tracking_id, 0, &outcome.message);
            }
        }

        Ok(outcome)
    }

    /// 实际执行同步操作
    async fn do_sync(&self, tracking: &tracking::Model) -> Result<SyncResult> {
        info!(tracking_id = tracking.id, "开始执行真实同步");

        let sync_service = SyncService::new(self.db);
        // 使用新的 Collector 接口进行同步
        // 注意：即使有注入的客户端，我们也使用 sync_tracking，
        // 因为它会根据 tracking 配置自动选择合适的 Collector
        let result = sync_service.sync_tracking(tracking.id).await;

        let sync_result = result.context("SyncService 同步失败")?;

        info!(
            tracking_id = tracking.id,
            commits_synced = sync_result.commits_synced,
            issues_synced = sync_result.issues_synced,
            status = ?sync_result.status,
            message = %sync_result.message,
            "同步执行完成"
        );

        Ok(sync_result)
    }

    /// 执行所有待处理的同步任务（默认限流）
    pub async fn execute_pending_tasks(&self) -> Result<SyncExecutionStats> {
        self.execute_pending_tasks_with_limit(DEFAULT_MAX_TASKS)
            .await
    }

    /// 执行待处理同步任务，最多处理 `max_tasks` 个
    pub async fn execute_pending_tasks_with_limit(
        &self,
        max_tasks: usize,
    ) -> Result<SyncExecutionStats> {
        let pending_tasks = self
            .sync_manager
            .get_pending_sync_tasks_ordered(false)
            .await
            .context("获取待处理任务失败")?;

        let mut stats = SyncExecutionStats {
            discovered: pending_tasks.len(),
            ..Default::default()
        };

        if max_tasks == 0 {
            return Ok(stats);
        }

        let limit = max_tasks.min(stats.discovered);
        for tracking in pending_tasks.into_iter().take(limit) {
            match self.execute_sync(tracking.id).await {
                Ok(outcome) => stats.record_outcome(tracking.id, &outcome),
                Err(err) => {
                    error!(
                        tracking_id = tracking.id,
                        error = %err,
                        "同步任务执行失败，继续处理下一个"
                    );
                    stats.record_error(tracking.id, err.to_string());
                }
            }
        }

        if stats.discovered > limit {
            info!(
                pending = stats.discovered - limit,
                "达到限流阈值，剩余同步任务保留至下次调度"
            );
        }

        Ok(stats)
    }

    /// 执行指定列表中的同步任务
    pub async fn execute_batch(&self, tracking_ids: Vec<i32>) -> SyncExecutionStats {
        let mut stats = SyncExecutionStats {
            discovered: tracking_ids.len(),
            ..Default::default()
        };

        for tracking_id in tracking_ids {
            match self.execute_sync(tracking_id).await {
                Ok(outcome) => stats.record_outcome(tracking_id, &outcome),
                Err(err) => stats.record_error(tracking_id, err.to_string()),
            }
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DatabaseBackend, MockDatabase};

    #[test]
    fn test_sync_executor_creation() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let executor = SyncExecutor::new(&db, None);

        // 验证执行器可以成功创建
        assert!(executor.client.is_none());
    }

    #[test]
    fn test_sync_execution_stats_default() {
        let stats = SyncExecutionStats::default();

        assert_eq!(stats.discovered, 0);
        assert_eq!(stats.processed, 0);
        assert_eq!(stats.succeeded, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.errors.len(), 0);
    }

    #[test]
    fn test_sync_execution_stats_record_success() {
        let mut stats = SyncExecutionStats::default();
        let outcome = SyncResult {
            status: SyncStatus::Success,
            commits_synced: 5,
            issues_synced: 2,
            message: "Success".to_string(),
        };

        stats.record_outcome(1, &outcome);

        assert_eq!(stats.processed, 1);
        assert_eq!(stats.succeeded, 1);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.skipped, 0);
    }

    #[test]
    fn test_sync_execution_stats_record_skipped() {
        let mut stats = SyncExecutionStats::default();
        let outcome = SyncResult {
            status: SyncStatus::Skipped,
            commits_synced: 0,
            issues_synced: 0,
            message: "Skipped".to_string(),
        };

        stats.record_outcome(1, &outcome);

        assert_eq!(stats.processed, 1);
        assert_eq!(stats.succeeded, 0);
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.failed, 0);
    }

    #[test]
    fn test_sync_execution_stats_record_failed() {
        let mut stats = SyncExecutionStats::default();
        let outcome = SyncResult {
            status: SyncStatus::Failed,
            commits_synced: 0,
            issues_synced: 0,
            message: "Error occurred".to_string(),
        };

        stats.record_outcome(1, &outcome);

        assert_eq!(stats.processed, 1);
        assert_eq!(stats.succeeded, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.errors.len(), 1);
        assert_eq!(stats.errors[0].0, 1);
        assert_eq!(stats.errors[0].1, "Error occurred");
    }

    #[test]
    fn test_sync_execution_stats_record_error() {
        let mut stats = SyncExecutionStats::default();

        stats.record_error(123, "Connection timeout".to_string());

        assert_eq!(stats.processed, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.errors.len(), 1);
        assert_eq!(stats.errors[0].0, 123);
        assert_eq!(stats.errors[0].1, "Connection timeout");
    }

    #[test]
    fn test_sync_execution_stats_multiple_records() {
        let mut stats = SyncExecutionStats::default();

        stats.record_outcome(
            1,
            &SyncResult {
                status: SyncStatus::Success,
                commits_synced: 5,
                issues_synced: 2,
                message: "OK".to_string(),
            },
        );

        stats.record_outcome(
            2,
            &SyncResult {
                status: SyncStatus::Skipped,
                commits_synced: 0,
                issues_synced: 0,
                message: "Skipped".to_string(),
            },
        );

        stats.record_error(3, "Failed".to_string());

        assert_eq!(stats.processed, 3);
        assert_eq!(stats.succeeded, 1);
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.errors.len(), 1);
    }

    #[tokio::test]
    async fn test_execute_batch_empty() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let executor = SyncExecutor::new(&db, None);

        let stats = executor.execute_batch(vec![]).await;

        assert_eq!(stats.discovered, 0);
        assert_eq!(stats.processed, 0);
    }

    #[tokio::test]
    async fn test_execute_pending_tasks_limit() {
        use crate::entities::tracking;
        use chrono::Utc;

        let tracking_model = tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/tmp/l2".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now() - chrono::Duration::hours(25)),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        // Mock pending tasks query
        // ... (rest of the comment)

        use crate::entities::{packages, sync_jobs};
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

        let inserted_job = sync_jobs::Model {
            id: 1,
            tracking_id: 1,
            job_kind: "sync".to_string(),
            scheduled_at: Utc::now(),
            started_at: None,
            finished_at: None,
            status: "pending".to_string(),
            error: None,
            attempt_count: 0,
            priority: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            // get_pending_sync_tasks_ordered -> returns (tracking, Some(package))
            .append_query_results::<(tracking::Model, Option<packages::Model>), _, _>(vec![vec![(
                tracking_model.clone(),
                Some(package_model.clone()),
            )]])
            // queue_sync_job -> find_active_sync_job -> none
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![]])
            // queue_sync_job -> insert returning
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![inserted_job]])
            .into_connection();

        let executor = SyncExecutor::new(&db, None);

        // Test limit 0：不会调用 execute_sync，避免复杂的 SyncService 依赖
        let stats = executor.execute_pending_tasks_with_limit(0).await.unwrap();
        assert_eq!(stats.discovered, 1);
        assert_eq!(stats.processed, 0);
    }
}
