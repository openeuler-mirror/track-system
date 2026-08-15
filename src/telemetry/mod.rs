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

    pub fn l0_poll_summary(package_id: i32, inserted: usize, skipped: usize) {
        info!(
            target = "l0",
            package_id, inserted, skipped, "l0 poll finished"
        );
    }

    pub fn backport_candidates_created(package_id: i32, created: usize, skipped: usize) {
        info!(
            target = "backport",
            package_id, created, skipped, "backport advisor summary"
        );
    }

    pub fn snapshot_export_completed(
        tracking_id: Option<i32>,
        path: &str,
        exported_at: DateTime<Utc>,
    ) {
        info!(target = "snapshot", tracking_id, %exported_at, path, "snapshot export completed");
    }

    pub fn pipeline_stage_completed(tracking_id: i32, stage_name: &str, success: bool) {
        if success {
            info!(
                target = "pipeline",
                tracking_id, stage_name, "pipeline stage completed successfully"
            );
        } else {
            warn!(
                target = "pipeline",
                tracking_id, stage_name, "pipeline stage failed"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn init_test_subscriber() {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    }

    #[test]
    fn sync_job_events_cover_success_and_failure_paths() {
        init_test_subscriber();

        Telemetry::sync_job_queued(1, 10, 5);
        Telemetry::sync_job_started(1, 10, 2);
        Telemetry::sync_job_completed(1, 10, true);
        Telemetry::sync_job_completed(1, 10, false);
        Telemetry::sync_job_succeeded(1, 10);
        Telemetry::sync_job_failed(1, 10, "network timeout");
    }

    #[test]
    fn domain_batch_events_accept_expected_counters() {
        init_test_subscriber();

        Telemetry::ingestion_batch_finished(7, 12, 3);
        Telemetry::l0_poll_summary(3, 4, 1);
        Telemetry::backport_candidates_created(8, 2, 5);
    }

    #[test]
    fn classification_events_cover_targeted_and_global_batches() {
        init_test_subscriber();

        Telemetry::classification_batch_processed(Some(42), 9);
        Telemetry::classification_batch_processed(None, 4);
    }

    #[test]
    fn snapshot_and_pipeline_events_cover_optional_and_failure_paths() {
        init_test_subscriber();
        let exported_at = Utc.with_ymd_and_hms(2026, 6, 23, 2, 58, 0).unwrap();

        Telemetry::snapshot_export_completed(Some(11), "/tmp/snapshot.json", exported_at);
        Telemetry::snapshot_export_completed(None, "/tmp/all-snapshots.json", exported_at);
        Telemetry::pipeline_stage_completed(11, "diff-comparison", true);
        Telemetry::pipeline_stage_completed(11, "report-generation", false);
    }
}
