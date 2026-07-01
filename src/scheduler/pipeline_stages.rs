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

//! 流水线各阶段的具体实现

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};

use crate::analyzer::ChangeClassifier;
use crate::backport_advisor::BackportAdvisor;
use crate::diff;
use crate::entities::{l1_commit_records, prelude::*, tracking, tracking_reports};
use crate::metadata_bridge;

use super::pipeline_executor::{
    BackportSuggestionResult, ClassificationResult, DiffComparisonResult, L1IngestionResult,
    L2SnapshotResult, PipelineExecutor, PipelineStage, ReportGenerationResult, StageResult,
};
use super::{SyncService, SyncStatus};

#[derive(Debug, Clone, Serialize)]
struct RiskCreateReq {
    #[serde(rename = "description")]
    description: String,
    #[serde(rename = "level")]
    level: i32,
    #[serde(rename = "reporter")]
    reporter: String,
    #[serde(rename = "type")]
    r#type: String,
    #[serde(rename = "software")]
    software: String,
    #[serde(rename = "version")]
    version: String,
    #[serde(rename = "release")]
    release: String,
    #[serde(rename = "platform")]
    platform: String,
    #[serde(rename = "disclosure_time", skip_serializing_if = "Option::is_none")]
    disclosure_time: Option<String>,
    #[serde(rename = "source", skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(rename = "package_id")]
    package_id: u64,
    #[serde(rename = "inner_secret")]
    inner_secret: String,
}

impl<'a> PipelineExecutor<'a> {
    /// 阶段 1: L1 元数据获取
    pub(super) async fn stage_l1_ingestion(
        &self,
        tracking: &tracking::Model,
    ) -> Result<L1IngestionResult> {
        info!(tracking_id = tracking.id, "执行 L1 元数据获取阶段");

        let sync_service = SyncService::new(self.db);

        // 使用新的 Collector 接口进行同步
        // 注意：即使有注入的客户端，我们也使用 sync_tracking，
        // 因为它会根据 tracking 配置自动选择合适的 Collector
        let sync_result = sync_service.sync_tracking(tracking.id).await?;

        // 检查同步状态
        let has_new_data = match sync_result.status {
            SyncStatus::Success => sync_result.commits_synced > 0 || sync_result.issues_synced > 0,
            SyncStatus::Skipped => false,
            SyncStatus::Failed => {
                return Err(anyhow::anyhow!("L1 同步失败: {}", sync_result.message));
            }
        };

        if !has_new_data {
            info!(tracking_id = tracking.id, "L1 没有新数据，可以跳过后续阶段");
        }

        // 如果有新数据，生成并持久化 L1 快照
        let (snapshot_path, snapshot_checksum) = if has_new_data {
            let output_path = format!(
                "/tmp/l1_snapshot_{}_{}.json",
                tracking.id,
                Utc::now().timestamp()
            );

            info!(
                tracking_id = tracking.id,
                repo_path = tracking.l1_repo_name,
                "开始导出 L1 快照"
            );
            let summary =
                metadata_bridge::export_l1_snapshot(self.db, tracking.id, None, &output_path)
                    .await
                    .context("导出 L1 快照失败")?;

            info!(
                tracking_id = tracking.id,
                commit_count = summary.commit_count,
                issue_count = summary.issue_count,
                repo_path = tracking.l1_repo_name,
                "L1 快照生成成功"
            );

            (Some(output_path), Some(summary.checksum))
        } else {
            (None, None)
        };

        Ok(L1IngestionResult {
            commits_synced: sync_result.commits_synced,
            issues_synced: sync_result.issues_synced,
            has_new_data,
            snapshot_path,
            snapshot_checksum,
        })
    }

    /// 阶段 2: L2 快照生成
    pub(super) async fn stage_l2_snapshot(
        &self,
        tracking: &tracking::Model,
    ) -> Result<L2SnapshotResult> {
        info!(
            tracking_id = tracking.id,
            repo_path = tracking.l2_repo_path,
            "执行 L2 快照生成阶段"
        );

        // 检查 L2 仓库路径是否存在
        let l2_repo_path = PathBuf::from(&tracking.l2_repo_path);
        if !l2_repo_path.exists() {
            warn!(
                tracking_id = tracking.id,
                path = %tracking.l2_repo_path,
                "不存在，尝试使用数据库中的历史快照"
            );

            // 查询数据库中最新的 L2 快照
            use crate::entities::l2_snapshots;
            use crate::entities::prelude::L2Snapshots;

