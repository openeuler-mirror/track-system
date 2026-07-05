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
