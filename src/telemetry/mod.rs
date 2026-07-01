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

use chrono::{DateTime, Utc};
use tracing::{info, warn};

/// 统一的遥测事件入口
pub struct Telemetry;

impl Telemetry {
    pub fn sync_job_queued(tracking_id: i32, job_id: i64, priority: i32) {
        info!(
            target = "sync",
            tracking_id, job_id, priority, "sync job queued"
        );
    }

    pub fn sync_job_started(tracking_id: i32, job_id: i64, attempt: i32) {
        info!(
            target = "sync",
            tracking_id, job_id, attempt, "sync job started"
        );
    }

    pub fn sync_job_completed(tracking_id: i32, job_id: i64, success: bool) {
        if success {
            info!(target = "sync", tracking_id, job_id, "sync job succeeded");
        } else {
            warn!(target = "sync", tracking_id, job_id, "sync job failed");
        }
    }

    pub fn sync_job_succeeded(tracking_id: i32, job_id: i64) {
        info!(target = "sync", tracking_id, job_id, "sync job succeeded");
    }

    pub fn sync_job_failed(tracking_id: i32, job_id: i64, error: &str) {
        warn!(
            target = "sync",
            tracking_id, job_id, error, "sync job failed"
        );
    }

    pub fn ingestion_batch_finished(tracking_id: i32, inserted: usize, skipped: usize) {
        info!(
            target = "ingest",
            tracking_id, inserted, skipped, "ingestion batch finished"
        );
    }

    pub fn classification_batch_processed(tracking_id: Option<i32>, processed: usize) {
        match tracking_id {
            Some(id) => info!(
                target = "classification",
                tracking_id = id,
                processed,
                "classification batch processed"
            ),
            None => info!(
                target = "classification",
                processed, "classification batch processed"
            ),
        }
    }
