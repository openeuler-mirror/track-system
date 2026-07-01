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

//! 同步任务管理器
//!
//! 负责管理跟踪配置的同步任务状态和调度

use chrono::Utc;
use sea_orm::*;
use tracing::info;

use super::{SyncResult, SyncStatus};
use crate::entities::{
    packages,
    prelude::*,
    sync_jobs::{self, Entity as SyncJobsEntity},
    tracking,
};
use crate::telemetry::Telemetry;

const SYNC_JOB_KIND: &str = "sync";
const STATUS_PENDING: &str = "pending";
const STATUS_RUNNING: &str = "running";
const STATUS_SUCCEEDED: &str = "succeeded";
const STATUS_FAILED: &str = "failed";

enum CompletionOutcome {
    Success,
    Failure { message: String },
    Skipped { reason: String },
}

/// 同步任务管理器
pub struct SyncManager<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> SyncManager<'a> {
    /// 创建新的同步管理器
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// 手动入队同步作业；如果已有待处理或运行中的作业则复用
    pub async fn queue_sync_job(
        &self,
        tracking_id: i32,
        priority: i32,
    ) -> anyhow::Result<sync_jobs::Model> {
        if let Some(existing) = self.find_active_sync_job(tracking_id).await? {
            return Ok(existing);
        }

        let tracking = Tracking::find_by_id(tracking_id)
            .one(self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Tracking {} not found", tracking_id))?;
        if matches!(tracking.tracking_status.as_str(), "paused" | "archived") {
            return Err(anyhow::anyhow!(
                "Tracking {} is {}, cannot queue sync job",
                tracking_id,
                tracking.tracking_status
            ));
        }

        let now = Utc::now();
        let job = sync_jobs::ActiveModel {
            tracking_id: Set(tracking_id),
            job_kind: Set(SYNC_JOB_KIND.to_string()),
            scheduled_at: Set(now),
            started_at: Set(None),
            finished_at: Set(None),
            status: Set(STATUS_PENDING.to_string()),
            error: Set(None),
            attempt_count: Set(0),
            priority: Set(priority),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let inserted = job.insert(self.db).await?;
        Telemetry::sync_job_queued(tracking_id, inserted.id, priority);
        Ok(inserted)
    }

    /// 更新跟踪配置的上次同步时间
    pub async fn update_last_sync(
        &self,
        tracking_id: i32,
        last_sync: chrono::DateTime<Utc>,
    ) -> anyhow::Result<()> {
        let track = Tracking::find_by_id(tracking_id)
            .one(self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Tracking {} not found", tracking_id))?;

        let mut active: tracking::ActiveModel = track.into();
        active.last_sync_time = Set(Some(last_sync));
        active.updated_at = Set(Utc::now());
        active.update(self.db).await?;

        Ok(())
    }

    /// 计算下次同步时间
    pub async fn calculate_next_sync_time(
        &self,
        tracking_id: i32,
    ) -> anyhow::Result<chrono::DateTime<Utc>> {
        let track = Tracking::find_by_id(tracking_id)
            .one(self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Tracking {} not found", tracking_id))?;

        // 获取关联的软件包信息
        let package = track
            .find_related(Packages)
            .one(self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Package not found for tracking {}", tracking_id))?;

        let interval_hours = package.sync_interval_hours as i64;
        if interval_hours <= 0 {
            return Ok(package.created_at);
        }

        let interval = chrono::Duration::hours(interval_hours);
        let base_time = package.created_at;
        let reference_time = track.last_sync_time.unwrap_or(base_time - interval);
        let next_sync = next_aligned_after(base_time, interval, reference_time);

        Ok(next_sync)
    }

    /// 获取需要同步的任务列表
    pub async fn get_pending_sync_tasks(&self) -> anyhow::Result<Vec<tracking::Model>> {
        self.get_pending_sync_tasks_with_limit(None).await
    }

    /// 获取需要同步的任务列表（带限制）
    pub async fn get_pending_sync_tasks_with_limit(
        &self,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<tracking::Model>> {
        let now = Utc::now();

        // 查询所有跟踪配置及其关联的软件包
        let mut query = Tracking::find()
            .find_also_related(Packages)
            .filter(tracking::Column::TrackingStatus.ne("syncing"))
            .filter(tracking::Column::TrackingStatus.ne("paused"))
            .filter(tracking::Column::TrackingStatus.ne("archived"));

        if let Some(limit) = limit {
            query = query.limit(limit as u64);
        }

        let results = query.all(self.db).await?;

        // 过滤出需要同步的任务
        let mut pending_tasks = Vec::new();
        for (track, package_opt) in results {
            if let Some(package) = package_opt {
                // 检查是否需要同步
                if should_sync(&track, &package, now) {
                    pending_tasks.push(track);
                }
            }
        }

        Ok(pending_tasks)
    }

    pub async fn get_pending_sync_tasks_with_tracking_id(
        &self,
        wake_up: bool,
        tracking_id: Option<i32>,
    ) -> anyhow::Result<Vec<tracking::Model>> {
        let now = Utc::now();

        // 查询所有跟踪配置及其关联的软件包，按等级排序
        let results = Tracking::find()
            .find_also_related(Packages)
            .filter(tracking::Column::TrackingStatus.ne("syncing"))
            .filter(tracking::Column::TrackingStatus.ne("paused"))
            .filter(tracking::Column::TrackingStatus.ne("archived"))
            .order_by_asc(packages::Column::Level) // 等级越小越优先（1级 > 2级 > 3级）
            .all(self.db)
            .await?;

        // 过滤出需要同步的任务
        let mut pending_tasks = Vec::new();
        for (track, package_opt) in results {
            info!("Checking track {} with package {:?}", track.id, package_opt);
            if let Some(package) = package_opt {
                // 如果指定了 tracking_id，只处理匹配的任务
                // 否则，处理所有任务
                let id_matches = tracking_id.is_none_or(|id| track.id == id);
                if id_matches && (should_sync(&track, &package, now) || wake_up) {
                    pending_tasks.push(track);
                }
            }
        }

        Ok(pending_tasks)
    }

    /// 获取需要同步的任务列表（按优先级排序）
    pub async fn get_pending_sync_tasks_ordered(
        &self,
        wake_up: bool,
    ) -> anyhow::Result<Vec<tracking::Model>> {
        let now = Utc::now();

        // 查询所有跟踪配置及其关联的软件包，按等级排序
        let results = Tracking::find()
            .find_also_related(Packages)
            .filter(tracking::Column::TrackingStatus.ne("syncing"))
            .filter(tracking::Column::TrackingStatus.ne("paused"))
            .filter(tracking::Column::TrackingStatus.ne("archived"))
            .order_by_asc(packages::Column::Level) // 等级越小越优先（1级 > 2级 > 3级）
            .all(self.db)
            .await?;

        // 过滤出需要同步的任务
        let mut pending_tasks = Vec::new();
        for (track, package_opt) in results {
            info!("Checking track {} with package {:?}", track.id, package_opt);
            if let Some(package) = package_opt {
                if should_sync(&track, &package, now) || wake_up {
                    pending_tasks.push(track);
                }
            }
        }

        Ok(pending_tasks)
    }

    /// 开始同步任务
    pub async fn start_sync_task(&self, tracking_id: i32) -> anyhow::Result<()> {
        let track = Tracking::find_by_id(tracking_id)
            .one(self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Tracking {} not found", tracking_id))?;

        if matches!(track.tracking_status.as_str(), "paused" | "archived") {
            return Err(anyhow::anyhow!(
                "Tracking {} is {}, cannot start sync task",
                tracking_id,
                track.tracking_status
            ));
        }

        let job = self.ensure_sync_job(tracking_id).await?;
        let mut job_active: sync_jobs::ActiveModel = job.clone().into();
        let attempts = job.attempt_count + 1;
        job_active.status = Set(STATUS_RUNNING.to_string());
        job_active.started_at = Set(Some(Utc::now()));
        job_active.finished_at = Set(None);
        job_active.error = Set(None);
        job_active.attempt_count = Set(attempts);
        job_active.updated_at = Set(Utc::now());
        job_active.update(self.db).await?;
        Telemetry::sync_job_started(tracking_id, job.id, attempts);

        let mut active: tracking::ActiveModel = track.into();
        active.tracking_status = Set("syncing".to_string());
        active.updated_at = Set(Utc::now());
        active.update(self.db).await?;

        Ok(())
    }

    /// 完成同步任务
    pub async fn complete_sync_task(&self, tracking_id: i32, success: bool) -> anyhow::Result<()> {
        if success {
            self.apply_completion(tracking_id, CompletionOutcome::Success)
                .await
        } else {
            self.apply_completion(
                tracking_id,
                CompletionOutcome::Failure {
                    message: "Sync failed".to_string(),
                },
            )
            .await
        }
    }

    pub async fn complete_sync_task_with_result(
        &self,
        tracking_id: i32,
        result: &SyncResult,
    ) -> anyhow::Result<()> {
        match result.status {
            SyncStatus::Success => {
                self.apply_completion(tracking_id, CompletionOutcome::Success)
                    .await
            }
            SyncStatus::Failed => {
                self.apply_completion(
                    tracking_id,
                    CompletionOutcome::Failure {
                        message: result.message.clone(),
                    },
                )
                .await
            }
            SyncStatus::Skipped => {
                self.apply_completion(
                    tracking_id,
                    CompletionOutcome::Skipped {
                        reason: result.message.clone(),
                    },
                )
                .await
            }
        }
    }

    /// 获取跟踪配置
    pub async fn get_tracking(&self, tracking_id: i32) -> anyhow::Result<tracking::Model> {
        Tracking::find_by_id(tracking_id)
            .one(self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Tracking {} not found", tracking_id))
    }
}

impl<'a> SyncManager<'a> {
    async fn apply_completion(
        &self,
        tracking_id: i32,
        outcome: CompletionOutcome,
    ) -> anyhow::Result<()> {
        let track = Tracking::find_by_id(tracking_id)
            .one(self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Tracking {} not found", tracking_id))?;

        let preserve_status = matches!(track.tracking_status.as_str(), "paused" | "archived");
        let mut active: tracking::ActiveModel = track.clone().into();
        let now = Utc::now();

        match &outcome {
            CompletionOutcome::Success => {
                if !preserve_status {
                    active.tracking_status = Set("active".to_string());
                }
                active.last_sync_time = Set(Some(now));
                active.last_error = Set(None);
            }
            CompletionOutcome::Failure { message } => {
                if !preserve_status {
                    active.tracking_status = Set("error".to_string());
                }
                active.last_error = Set(Some(message.clone()));
            }
            CompletionOutcome::Skipped { reason } => {
                if !preserve_status {
                    active.tracking_status = Set("active".to_string());
                }
                active.last_error = Set(Some(reason.clone()));
                // 保持 last_sync_time 不变
            }
        }

        active.updated_at = Set(now);
        active.update(self.db).await?;

        if let Some(job) = self.find_latest_sync_job(tracking_id).await? {
            let job_id = job.id;
            let mut job_active: sync_jobs::ActiveModel = job.into();
            match &outcome {
                CompletionOutcome::Success => {
                    job_active.status = Set(STATUS_SUCCEEDED.to_string());
                    job_active.error = Set(None);
                }
                CompletionOutcome::Failure { message } => {
                    job_active.status = Set(STATUS_FAILED.to_string());
                    job_active.error = Set(Some(message.clone()));
                }
                CompletionOutcome::Skipped { reason } => {
                    job_active.status = Set(STATUS_SUCCEEDED.to_string());
                    job_active.error = Set(Some(reason.clone()));
                }
            }
            job_active.finished_at = Set(Some(now));
            job_active.updated_at = Set(now);
            job_active.update(self.db).await?;

            let telemetry_success = matches!(
                outcome,
                CompletionOutcome::Success | CompletionOutcome::Skipped { .. }
            );
            Telemetry::sync_job_completed(tracking_id, job_id, telemetry_success);
        }

        Ok(())
    }
}

impl<'a> SyncManager<'a> {
    async fn find_active_sync_job(
        &self,
        tracking_id: i32,
    ) -> anyhow::Result<Option<sync_jobs::Model>> {
        let job = SyncJobsEntity::find()
            .filter(sync_jobs::Column::TrackingId.eq(tracking_id))
            .filter(sync_jobs::Column::JobKind.eq(SYNC_JOB_KIND))
            .filter(sync_jobs::Column::Status.is_in(vec![STATUS_PENDING, STATUS_RUNNING]))
            .order_by_desc(sync_jobs::Column::ScheduledAt)
            .one(self.db)
            .await?;

        Ok(job)
    }

    async fn find_latest_sync_job(
        &self,
        tracking_id: i32,
    ) -> anyhow::Result<Option<sync_jobs::Model>> {
        let job = SyncJobsEntity::find()
            .filter(sync_jobs::Column::TrackingId.eq(tracking_id))
            .filter(sync_jobs::Column::JobKind.eq(SYNC_JOB_KIND))
            .order_by_desc(sync_jobs::Column::ScheduledAt)
            .one(self.db)
            .await?;

        Ok(job)
    }

    async fn ensure_sync_job(&self, tracking_id: i32) -> anyhow::Result<sync_jobs::Model> {
        if let Some(job) = self.find_active_sync_job(tracking_id).await? {
            return Ok(job);
        }

        let now = Utc::now();
        let job = sync_jobs::ActiveModel {
            tracking_id: Set(tracking_id),
            job_kind: Set(SYNC_JOB_KIND.to_string()),
            scheduled_at: Set(now),
            started_at: Set(None),
            finished_at: Set(None),
            status: Set(STATUS_PENDING.to_string()),
            error: Set(None),
            attempt_count: Set(0),
            priority: Set(0),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let inserted = job.insert(self.db).await?;
        Telemetry::sync_job_queued(tracking_id, inserted.id, 0);
        Ok(inserted)
    }
}

/// 判断是否需要同步
fn should_sync(
    track: &tracking::Model,
    package: &packages::Model,
    now: chrono::DateTime<Utc>,
