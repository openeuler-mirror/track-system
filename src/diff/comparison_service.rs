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

//! L1/L2仓库对比服务
//!
//! 负责对比L1（上游）和L2（本地）仓库的差异

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};
use std::path::Path;
use tokio::task;
use tracing::{debug, info};

use super::git_client::GitRepositoryClient;
use crate::entities::{prelude::TrackingReports, tracking, tracking_reports};

/// 差异对比摘要
#[derive(Debug, Clone)]
pub struct ComparisonReport {
    /// 追踪配置ID
    pub tracking_id: i32,
    /// L1落后的commit数
    pub commits_behind: usize,
    /// L1领先的commit数
    pub commits_ahead: usize,
    /// 差异摘要（JSON）
    pub diff_summary: serde_json::Value,
    /// 生成源
    pub source: String,
}

/// L1/L2仓库对比服务
pub struct ComparisonService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> ComparisonService<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// 生成对比报告（快速版，基于 SHA）
    pub async fn generate_report(&self, tracking: &tracking::Model) -> Result<ComparisonReport> {
        let mut report = ComparisonReport {
            tracking_id: tracking.id,
            commits_behind: 0,
            commits_ahead: 0,
            diff_summary: json!({}),
            source: "auto".to_string(),
        };

        // 基于tracking配置计算差异
        if let (Some(l1_sha), Some(l2_sha)) =
            (&tracking.last_l1_commit_sha, &tracking.last_l2_commit_sha)
        {
            // 如果L1和L2的commit不同，说明有差异
            if l1_sha != l2_sha {
                report.commits_ahead = 1;
            }
        } else if tracking.last_l1_commit_sha.is_some() && tracking.last_l2_commit_sha.is_none() {
            // L1有新commit但L2没有同步
            report.commits_ahead = 1;
        }

        // 构建详细的diff摘要
        report.diff_summary = json!({
            "tracking_id": tracking.id,
            "l1_latest_sha": tracking.last_l1_commit_sha.as_deref().unwrap_or("unknown"),
            "l2_latest_sha": tracking.last_l2_commit_sha.as_deref().unwrap_or("unknown"),
            "commits_ahead": report.commits_ahead,
            "commits_behind": report.commits_behind,
            "needs_sync": report.commits_ahead > 0,
            "generated_at": Utc::now().to_rfc3339(),
            "method": "sha_comparison",
        });

        Ok(report)
    }

    /// 对比 L1 和 L2 仓库的 commits（完整版，基于实际 Git 历史）
    pub async fn compare_l1_l2_git(
        &self,
        tracking: &tracking::Model,
        l2_repo_path: impl AsRef<Path>,
    ) -> Result<ComparisonReport> {
        let l2_path = l2_repo_path.as_ref();

        debug!(
            tracking_id = tracking.id,
            l2_path = ?l2_path,
            "开始 L1/L2 Git 对比"
        );

        // 在线程池中执行 Git 操作（避免阻塞异步任务）
        let l1_branch = tracking.l1_branch.clone();
        let l2_branch = tracking.l2_branch.clone();
        let l2_path = l2_path.to_path_buf();

        let git_client = task::spawn_blocking(move || GitRepositoryClient::new(&l2_path)).await??;

        // 对比分支
        let diff =
            task::spawn_blocking(move || git_client.compare_branches(&l1_branch, &l2_branch))
                .await??;

        let report = ComparisonReport {
            tracking_id: tracking.id,
            commits_behind: diff.l2_ahead.len(),
            commits_ahead: diff.l1_ahead.len(),
            diff_summary: self.build_detailed_diff(tracking, &diff, "git_comparison")?,
            source: "git_diff".to_string(),
        };

        info!(
            tracking_id = tracking.id,
            commits_ahead = diff.l1_ahead.len(),
            commits_behind = diff.l2_ahead.len(),
            "L1/L2 Git 对比完成"
        );

        Ok(report)
    }

    /// 构建详细的差异信息
    fn build_detailed_diff(
        &self,
        tracking: &tracking::Model,
        diff: &super::git_client::CommitDiff,
        method: &str,
    ) -> Result<Value> {
        let l1_commits: Vec<_> = diff
            .l1_ahead
            .iter()
            .map(|c| {
                json!({
                    "sha": &c.sha,
                    "message": &c.message,
                    "author": &c.author,
                    "committed_at": c.committed_at.to_rfc3339(),
                    "files_changed": c.files_changed,
                })
            })
            .collect();

        let l2_commits: Vec<_> = diff
            .l2_ahead
            .iter()
            .map(|c| {
                json!({
                    "sha": &c.sha,
                    "message": &c.message,
                    "author": &c.author,
                    "committed_at": c.committed_at.to_rfc3339(),
                    "files_changed": c.files_changed,
                })
            })
            .collect();

        Ok(json!({
            "tracking_id": tracking.id,
            "method": method,
            "l1_repo": format!("{}/{}", tracking.l1_repo_owner, tracking.l1_repo_name),
            "l1_branch": &tracking.l1_branch,
            "l2_branch": &tracking.l2_branch,
            "commits_ahead": {
                "count": diff.l1_ahead.len(),
                "commits": l1_commits,
            },
            "commits_behind": {
                "count": diff.l2_ahead.len(),
                "commits": l2_commits,
            },
            "summary": {
                "l1_ahead": diff.l1_ahead.len(),
                "l2_ahead": diff.l2_ahead.len(),
                "needs_backport": !diff.l1_ahead.is_empty(),
                "needs_forward_port": !diff.l2_ahead.is_empty(),
            },
            "generated_at": Utc::now().to_rfc3339(),
        }))
    }

    /// 保存对比报告到数据库
    pub async fn save_report(&self, report: &ComparisonReport) -> Result<()> {
        let now = Utc::now();

        let active = tracking_reports::ActiveModel {
            tracking_id: Set(report.tracking_id),
            generated_at: Set(now),
            diff_summary: Set(report.diff_summary.clone()),
            representative_changes: Set(None),
            source: Set(report.source.clone()),
            status: Set("completed".to_string()),
            failure_reason: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        active.insert(self.db).await.context("保存对比报告失败")?;

        info!(
            tracking_id = report.tracking_id,
            commits_ahead = report.commits_ahead,
            commits_behind = report.commits_behind,
            "对比报告已保存"
        );

        Ok(())
    }

    /// 获取最新的对比报告
    pub async fn get_latest_report(
        &self,
        tracking_id: i32,
    ) -> Result<Option<tracking_reports::Model>> {
        let report = TrackingReports::find()
            .filter(tracking_reports::Column::TrackingId.eq(tracking_id))
            .order_by_desc(tracking_reports::Column::GeneratedAt)
            .one(self.db)
            .await?;

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::git_client::{CommitDiff, GitCommit};
    use chrono::Utc;
    use sea_orm::{DatabaseBackend, MockDatabase};

    fn create_test_tracking_model() -> tracking::Model {
        tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "upstream".to_string(),
            l1_repo_name: "test-repo".to_string(),
            l2_branch: "openeuler".to_string(),
            l2_repo_path: "/tmp/test-repo".to_string(),
            tracking_status: "active".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: Some("abc123".to_string()),
            last_l2_commit_sha: Some("def456".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        }
    }

    fn create_test_git_commit(sha: &str, message: &str, author: &str) -> GitCommit {
        GitCommit {
            sha: sha.to_string(),
            message: message.to_string(),
            author: author.to_string(),
            author_email: format!("{}@example.com", author),
            committed_at: Utc::now(),
            files_changed: 5,
        }
    }

    #[tokio::test]
    async fn test_new() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let service = ComparisonService::new(&db);
        // Verify service is created successfully
        assert!(std::ptr::addr_of!(service) as usize != 0);
    }

    #[tokio::test]
    async fn test_generate_report_same_sha() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let service = ComparisonService::new(&db);

        let mut tracking = create_test_tracking_model();
        tracking.last_l1_commit_sha = Some("same_sha".to_string());
        tracking.last_l2_commit_sha = Some("same_sha".to_string());

        let report = service.generate_report(&tracking).await.unwrap();

        assert_eq!(report.tracking_id, 1);
        assert_eq!(report.commits_ahead, 0);
        assert_eq!(report.commits_behind, 0);
        assert_eq!(report.source, "auto");
        assert_eq!(report.diff_summary["tracking_id"], 1);
        assert_eq!(report.diff_summary["needs_sync"], false);
        assert_eq!(report.diff_summary["method"], "sha_comparison");
    }

    #[tokio::test]
    async fn test_generate_report_different_sha() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let service = ComparisonService::new(&db);

        let mut tracking = create_test_tracking_model();
        tracking.last_l1_commit_sha = Some("sha1".to_string());
        tracking.last_l2_commit_sha = Some("sha2".to_string());

        let report = service.generate_report(&tracking).await.unwrap();

        assert_eq!(report.tracking_id, 1);
        assert_eq!(report.commits_ahead, 1);
        assert_eq!(report.commits_behind, 0);
        assert_eq!(report.diff_summary["needs_sync"], true);
        assert_eq!(report.diff_summary["l1_latest_sha"], "sha1");
        assert_eq!(report.diff_summary["l2_latest_sha"], "sha2");
    }

    #[tokio::test]
    async fn test_generate_report_l1_exists_l2_none() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let service = ComparisonService::new(&db);

        let mut tracking = create_test_tracking_model();
        tracking.last_l1_commit_sha = Some("sha1".to_string());
        tracking.last_l2_commit_sha = None;

        let report = service.generate_report(&tracking).await.unwrap();

        assert_eq!(report.commits_ahead, 1);
        assert_eq!(report.diff_summary["needs_sync"], true);
        assert_eq!(report.diff_summary["l2_latest_sha"], "unknown");
    }

    #[tokio::test]
    async fn test_generate_report_both_none() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let service = ComparisonService::new(&db);

        let mut tracking = create_test_tracking_model();
        tracking.last_l1_commit_sha = None;
        tracking.last_l2_commit_sha = None;

        let report = service.generate_report(&tracking).await.unwrap();

        assert_eq!(report.commits_ahead, 0);
        assert_eq!(report.commits_behind, 0);
        assert_eq!(report.diff_summary["needs_sync"], false);
        assert_eq!(report.diff_summary["l1_latest_sha"], "unknown");
        assert_eq!(report.diff_summary["l2_latest_sha"], "unknown");
    }

    #[test]
    fn test_build_detailed_diff() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let service = ComparisonService::new(&db);
        let tracking = create_test_tracking_model();

        let commit1 = create_test_git_commit("sha1", "Add feature A", "alice");
        let commit2 = create_test_git_commit("sha2", "Fix bug B", "bob");
        let commit3 = create_test_git_commit("sha3", "Update docs", "charlie");

