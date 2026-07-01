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
