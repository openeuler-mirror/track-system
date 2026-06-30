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

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, JsonValue, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde_json::json;

use crate::{
    analyzer::ChangeClassifier,
    entities::{l1_commit_records, prelude::L1CommitRecords, prelude::SyncJobs, sync_jobs},
    telemetry::Telemetry,
};

const CLASSIFICATION_JOB_KIND: &str = "classification";
const STATUS_PENDING: &str = "pending";
const STATUS_RUNNING: &str = "running";
const STATUS_SUCCEEDED: &str = "succeeded";
const STATUS_FAILED: &str = "failed";

/// 变更分类任务队列
pub struct ClassificationJobQueue<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> ClassificationJobQueue<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// 入队一个给定 tracking 的分类任务，如果已有待处理任务则复用
    pub async fn enqueue(&self, tracking_id: i32) -> Result<()> {
        let existing = SyncJobs::find()
            .filter(sync_jobs::Column::TrackingId.eq(tracking_id))
            .filter(sync_jobs::Column::JobKind.eq(CLASSIFICATION_JOB_KIND))
            .filter(sync_jobs::Column::Status.is_in(vec![STATUS_PENDING, STATUS_RUNNING]))
            .one(self.db)
            .await?;

        if existing.is_some() {
            return Ok(());
        }

        let now = Utc::now();
        let job = sync_jobs::ActiveModel {
            tracking_id: Set(tracking_id),
            job_kind: Set(CLASSIFICATION_JOB_KIND.to_string()),
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

        job.insert(self.db).await?;
        Ok(())
    }

    /// 抓取下一批待执行的分类任务
    pub async fn fetch_pending_jobs(&self, limit: usize) -> Result<Vec<sync_jobs::Model>> {
        let jobs = SyncJobs::find()
            .filter(sync_jobs::Column::JobKind.eq(CLASSIFICATION_JOB_KIND))
            .filter(sync_jobs::Column::Status.eq(STATUS_PENDING))
            .order_by_desc(sync_jobs::Column::Priority)
            .order_by_asc(sync_jobs::Column::ScheduledAt)
            .limit(limit as u64)
            .all(self.db)
            .await?;

        Ok(jobs)
    }

    /// 将任务标记为运行中
