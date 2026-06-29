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
    pub async fn mark_started(&self, job: &sync_jobs::Model) -> Result<sync_jobs::Model> {
        let mut active: sync_jobs::ActiveModel = job.clone().into();
        active.status = Set(STATUS_RUNNING.to_string());
        active.started_at = Set(Some(Utc::now()));
        active.finished_at = Set(None);
        active.attempt_count = Set(job.attempt_count + 1);
        active.updated_at = Set(Utc::now());

        Ok(active.update(self.db).await?)
    }

    /// 成功结束任务
    pub async fn mark_succeeded(&self, job: &sync_jobs::Model) -> Result<sync_jobs::Model> {
        let mut active: sync_jobs::ActiveModel = job.clone().into();
        active.status = Set(STATUS_SUCCEEDED.to_string());
        active.finished_at = Set(Some(Utc::now()));
        active.error = Set(None);
        active.updated_at = Set(Utc::now());

        Ok(active.update(self.db).await?)
    }

    /// 任务失败，记录错误信息
    pub async fn mark_failed(
        &self,
        job: &sync_jobs::Model,
        error: &str,
    ) -> Result<sync_jobs::Model> {
        let mut active: sync_jobs::ActiveModel = job.clone().into();
        active.status = Set(STATUS_FAILED.to_string());
        active.finished_at = Set(Some(Utc::now()));
        active.error = Set(Some(error.to_string()));
        active.updated_at = Set(Utc::now());

        Ok(active.update(self.db).await?)
    }

    /// 获取下一条待执行任务并标记为运行中
    pub async fn dequeue(&self) -> Result<Option<sync_jobs::Model>> {
        let jobs = self.fetch_pending_jobs(1).await?;

        if let Some(job) = jobs.into_iter().next() {
            let running = self.mark_started(&job).await?;
            return Ok(Some(running));
        }

        Ok(None)
    }
}

pub struct ClassificationWorker<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> ClassificationWorker<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// 处理待分类的 commit 记录
    pub async fn process_pending(&self, limit: usize) -> Result<usize> {
        self.process_internal(None, limit).await
    }

    /// 仅处理指定 tracking 的待分类记录
    pub async fn process_tracking(&self, tracking_id: i32, limit: usize) -> Result<usize> {
        self.process_internal(Some(tracking_id), limit).await
    }

    async fn process_internal(&self, tracking_id: Option<i32>, limit: usize) -> Result<usize> {
        let mut query = L1CommitRecords::find()
            .filter(l1_commit_records::Column::ClassificationStatus.eq(STATUS_PENDING))
            .order_by_asc(l1_commit_records::Column::CommittedAt)
            .limit(limit as u64);

        if let Some(tracking_id) = tracking_id {
            query = query.filter(l1_commit_records::Column::TrackingId.eq(tracking_id));
        }

        let pending = query.all(self.db).await?;

        if pending.is_empty() {
            return Ok(0);
        }

        let classifier = ChangeClassifier::new(self.db);
        let mut processed = 0usize;

        for record in pending {
            let result = classifier.classify_commit(record.id).await;
            let mut active: l1_commit_records::ActiveModel = record.into();

            match result {
                Ok(classification) => {
                    let patch_stats = json!({
                        "added": classification.patch_changes.added,
                        "deleted": classification.patch_changes.deleted,
                        "modified": classification.patch_changes.modified,
                    });

                    let cve_json: Option<JsonValue> = if classification.cve_numbers.is_empty() {
                        None
                    } else {
                        Some(serde_json::to_value(classification.cve_numbers.clone())?)
                    };

                    active.primary_change_type =
                        Set(Some(classification.primary_type.as_str().to_string()));
                    active.spec_changed = Set(classification.has_spec_change);
                    active.patch_stats = Set(Some(patch_stats));
                    active.cve_list = Set(cve_json);
                    active.classification_status = Set("done".to_string());
                    active.classification_notes = Set(None);
                }
                Err(err) => {
                    active.classification_status = Set("needs_review".to_string());
                    active.classification_notes = Set(Some(err.to_string()));
                }
            }

            active.updated_at = Set(Utc::now());
            active.save(self.db).await?;
            processed += 1;
        }

        Telemetry::classification_batch_processed(tracking_id, processed);
        Ok(processed)
    }
}

/// 分类流水线执行器
pub struct ClassificationJobRunner<'a> {
    queue: ClassificationJobQueue<'a>,
