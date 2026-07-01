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

            let l2_record = L2Snapshots::find()
                .filter(l2_snapshots::Column::TrackingId.eq(tracking.id))
                .filter(l2_snapshots::Column::SnapshotType.eq("l2"))
                .order_by_desc(l2_snapshots::Column::CreatedAt)
                .one(self.db)
                .await?;

            if let Some(snapshot) = l2_record {
                // 反序列化快照以获取文件数量
                let snapshot_data: crate::snapshot::types::RepositorySnapshot =
                    serde_json::from_value(snapshot.payload.clone())
                        .context("解析 L2 快照 payload 失败")?;

                info!(
                    tracking_id = tracking.id,
                    snapshot_id = snapshot.id,
                    files_count = snapshot_data.files.len(),
                    created_at = %snapshot.created_at,
                    "使用数据库中的历史 L2 快照"
                );

                return Ok(L2SnapshotResult {
                    snapshot_id: Some(snapshot.id as i64),
                    snapshot_path: None,
                    files_count: snapshot_data.files.len(),
                    has_new_data: true,
                });
            } else {
                warn!(
                    tracking_id = tracking.id,
                    "数据库中也不存在 L2 快照，跳过快照生成"
                );
                return Ok(L2SnapshotResult {
                    snapshot_id: None,
                    snapshot_path: None,
                    files_count: 0,
                    has_new_data: false,
                });
            }
        }

        // 生成临时输出路径
        let output_path = format!(
            "/tmp/l2_snapshot_{}_{}.json",
            tracking.id,
            Utc::now().timestamp()
        );

        // 导出 L2 快照
        let summary =
            metadata_bridge::export_l2_snapshot(self.db, tracking.id, &l2_repo_path, &output_path)
                .await
                .context("导出 L2 快照失败")?;

        info!(
            tracking_id = tracking.id,
            files_count = summary.file_count,
            spec_version = ?summary.spec_version,
            "L2 快照生成成功"
        );

        Ok(L2SnapshotResult {
            snapshot_id: None, // metadata_bridge 已经持久化到数据库
            snapshot_path: Some(output_path),
            files_count: summary.file_count,
            has_new_data: true,
        })
    }

    /// 阶段 3: 差异对比
    pub(super) async fn stage_diff_comparison(
        &self,
        tracking: &tracking::Model,
        _previous_results: &HashMap<PipelineStage, StageResult>,
    ) -> Result<DiffComparisonResult> {
        info!(tracking_id = tracking.id, "执行差异对比阶段");

        // 1. 执行 L2 vs L1 对比（内容对比）
        let l2_vs_l1_result = self.compare_l2_vs_l1(tracking).await?;

        // 2. 执行 L1 vs L0 对比（版本对比）
        let l1_vs_l0_result = self.compare_l1_vs_l0(tracking).await?;

        // 3. 保存对比报告到数据库
        let report_id = self
            .save_comparison_reports(tracking, &l2_vs_l1_result, &l1_vs_l0_result)
            .await?;

        let files_changed = l2_vs_l1_result
            .as_ref()
            .map(|r| r.patch_diff.l2_added.len() + r.patch_diff.l2_modified.len())
            .unwrap_or(0);
        let has_spec_changes = l2_vs_l1_result
            .as_ref()
            .map(|r| !r.spec_diff.content_identical)
            .unwrap_or(false);

        info!(
            tracking_id = tracking.id,
            files_changed = files_changed,
            has_spec_changes = has_spec_changes,
            l1_vs_l0_completed = l1_vs_l0_result.is_some(),
            l2_vs_l1_completed = l2_vs_l1_result.is_some(),
            "差异对比完成"
        );

        Ok(DiffComparisonResult {
            report_id: Some(report_id),
            files_changed,
            has_spec_changes,
        })
    }

    /// 执行 L2 vs L1 对比
    async fn compare_l2_vs_l1(
        &self,
        tracking: &tracking::Model,
    ) -> Result<Option<diff::l2_vs_l1::L2VsL1Report>> {
        use crate::entities::l2_snapshots;
        use crate::entities::prelude::{L2Snapshots, Packages};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        info!(tracking_id = tracking.id, "执行 L2 vs L1 对比");

        // 获取软件包名称
        let package = Packages::find_by_id(tracking.package_id)
            .one(self.db)
            .await?
            .context("未找到关联的软件包记录")?;
        let package_name = package.name.clone();

        // 查询最新的 L1/L2 快照
        let l1_record = L2Snapshots::find()
            .filter(l2_snapshots::Column::TrackingId.eq(tracking.id))
            .filter(l2_snapshots::Column::SnapshotType.eq("l1"))
            .order_by_desc(l2_snapshots::Column::CreatedAt)
            .one(self.db)
            .await?;

        let l2_record = L2Snapshots::find()
            .filter(l2_snapshots::Column::TrackingId.eq(tracking.id))
            .filter(l2_snapshots::Column::SnapshotType.eq("l2"))
            .order_by_desc(l2_snapshots::Column::CreatedAt)
            .one(self.db)
            .await?;

        if l1_record.is_none() || l2_record.is_none() {
            warn!(
                tracking_id = tracking.id,
                has_l1 = l1_record.is_some(),
                has_l2 = l2_record.is_some(),
                "缺少 L1/L2 快照，跳过内容对比"
            );
            return Ok(None);
        }

        // 反序列化快照
        let l1_snapshot: crate::snapshot::types::RepositorySnapshot =
            serde_json::from_value(l1_record.as_ref().unwrap().payload.clone())
                .context("解析 L1 快照 payload 失败")?;
        let l2_snapshot: crate::snapshot::types::RepositorySnapshot =
            serde_json::from_value(l2_record.as_ref().unwrap().payload.clone())
                .context("解析 L2 快照 payload 失败")?;

        let comparator = diff::l2_vs_l1::L2VsL1Comparator::new();
        let l1_snap = diff::l2_vs_l1::L2VsL1Comparator::create_l1_snapshot(
            package_name.clone(),
            &l1_snapshot,
        )
        .context("构建 L1 快照失败")?;
        let l2_snap = diff::l2_vs_l1::L2VsL1Comparator::create_l2_snapshot(
            package_name.clone(),
            &l2_snapshot,
        )
        .context("构建 L2 快照失败")?;

        // 执行对比
        let report = comparator
            .compare(&l1_snap, &l2_snap, self.db, tracking.id)
            .await
            .context("L2 vs L1 内容对比失败")?;

        info!(
            tracking_id = tracking.id,
            l1_patches = l1_snap.patches.len(),
            l2_patches = l2_snap.patches.len(),
            has_spec_changes = !report.spec_diff.content_identical,
            "L2 vs L1 对比完成"
        );

        Ok(Some(report))
    }

    /// 执行 L1 vs L0 对比
    async fn compare_l1_vs_l0(
        &self,
        tracking: &tracking::Model,
    ) -> Result<Option<diff::l1_vs_l0::L1VsL0Report>> {
        use crate::diff::l1_vs_l0::L1VsL0Comparator;

        info!(tracking_id = tracking.id, "执行 L1 vs L0 对比");

        // 获取 L0 版本信息（从 l0_commits 表）
        let l0_info = self.get_l0_version_info(tracking).await?;
        if l0_info.is_none() {
            warn!(
                tracking_id = tracking.id,
                "缺少 L0 版本信息，跳过 L1 vs L0 对比"
            );
            return Ok(None);
        }

        // 获取 L1 版本信息（从 commit_records 和快照）
        let l1_info = self.get_l1_version_info(tracking).await?;
        if l1_info.is_none() {
            warn!(
                tracking_id = tracking.id,
                "缺少 L1 版本信息，跳过 L1 vs L0 对比"
            );
            return Ok(None);
        }

        // 使用 L1VsL0Comparator
        let comparator = L1VsL0Comparator::new();
        let report = comparator
            .compare(&l0_info.unwrap(), &l1_info.unwrap())
            .await
            .context("L1 vs L0 对比失败")?;

        info!(
            tracking_id = tracking.id,
            version_behind = report.version_behind,
            "L1 vs L0 对比完成"
        );

        Ok(Some(report))
    }

    /// 获取 L0 版本信息
    async fn get_l0_version_info(
        &self,
        tracking: &tracking::Model,
    ) -> Result<Option<diff::l1_vs_l0::L0VersionInfo>> {
        use crate::entities::{l0_commits, prelude::*};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        // 从 l0_commits 表获取版本信息
        let l0_commits = L0Commits::find()
            .filter(l0_commits::Column::PackageId.eq(tracking.package_id))
            .order_by_desc(l0_commits::Column::AuthoredAt)
            .all(self.db)
            .await?
            .into_iter()
            .take(100)
            .collect::<Vec<_>>();

        if l0_commits.is_empty() {
            return Ok(None);
        }

        // TODO: 从 l0_commits 构建 L0VersionInfo
        // 这里需要解析 commit message 和 tags 来提取版本信息
        // 暂时返回 None，需要进一步实现
        warn!(tracking_id = tracking.id, "L0 版本信息提取功能待实现");

        Ok(None)
    }

    /// 获取 L1 版本信息
    async fn get_l1_version_info(
        &self,
        tracking: &tracking::Model,
    ) -> Result<Option<diff::l1_vs_l0::L1VersionInfo>> {
        // TODO: 从 commit_records 和快照提取 L1 版本信息
        // 需要解析 spec 文件和 patch 文件
        warn!(tracking_id = tracking.id, "L1 版本信息提取功能待实现");

        Ok(None)
    }

    /// 保存对比报告
    async fn save_comparison_reports(
        &self,
        tracking: &tracking::Model,
        l2_vs_l1: &Option<diff::l2_vs_l1::L2VsL1Report>,
        l1_vs_l0: &Option<diff::l1_vs_l0::L1VsL0Report>,
    ) -> Result<i64> {
        use sea_orm::Set;

        // 构建报告摘要，包含详细的 patch 和 commit_diff 信息

        let l2_vs_l1_diff = l2_vs_l1.as_ref().map(|r| serde_json::json!({
                "patches_added": r.patch_diff.l2_added.len(),
                "patches_added_list": r.patch_diff.l2_added.iter().map(|p| serde_json::json!({
                    "filename": p.filename,
                    "path": p.path,
                    "content_hash": p.content_hash,
                    "size": p.size,
                    "applied": p.applied,
                })).collect::<Vec<_>>(),
                "patches_modified": r.patch_diff.l2_modified.len(),
                "patches_modified_list": r.patch_diff.l2_modified.iter().map(|p| serde_json::json!({
                    "filename": p.filename,
                    "l1_hash": p.l1_hash,
                    "l2_hash": p.l2_hash,
                })).collect::<Vec<_>>(),
                "patches_removed": r.patch_diff.l2_removed.len(),
                "patches_removed_list": r.patch_diff.l2_removed.iter().map(|p| serde_json::json!({
                    "filename": p.filename,
                    "path": p.path,
