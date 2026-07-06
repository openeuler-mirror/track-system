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

//! 元数据导入器
//!
//! 负责解析 track-collector 导出的 JSON 并导入到数据库

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

use crate::entities::{issues, l1_commit_records, prelude::*};

/// 导入的元数据（track-collector 格式）
#[derive(Debug, Serialize, Deserialize)]
pub struct CollectedMetadata {
    /// 平台类型（gitee/github）
    pub platform: String,
    /// 仓库所有者
    pub owner: String,
    /// 仓库名称
    pub repo: String,
    /// 分支名称
    pub branch: String,
    /// 采集时间
    pub collected_at: DateTime<Utc>,
    /// 仓库信息
    pub repository_info: Option<RepoInfo>,
    /// Commits
    pub commits: Vec<CommitInfo>,
    /// Issues
    pub issues: Vec<IssueInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepoInfo {
    pub description: Option<String>,
    pub stars: u32,
    pub forks: u32,
    pub open_issues: u32,
    pub default_branch: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitInfo {
    pub sha: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub author_date: DateTime<Utc>,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_date: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IssueInfo {
    pub number: i64,
    pub title: String,
    pub state: String,
    pub author: Option<String>,
    pub labels: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

/// 导入结果统计
#[derive(Debug)]
pub struct ImportResult {
    pub commits_imported: usize,
    pub commits_skipped: usize,
    pub issues_imported: usize,
    pub issues_skipped: usize,
    pub success: bool,
    pub message: String,
}

impl ImportResult {
    pub fn success(
        commits_imported: usize,
        commits_skipped: usize,
        issues_imported: usize,
        issues_skipped: usize,
    ) -> Self {
        Self {
            commits_imported,
            commits_skipped,
            issues_imported,
            issues_skipped,
            success: true,
            message: format!(
                "导入成功: commits={}/{}, issues={}/{}",
                commits_imported,
                commits_imported + commits_skipped,
                issues_imported,
                issues_imported + issues_skipped
            ),
        }
    }

    pub fn failed(message: String) -> Self {
        Self {
            commits_imported: 0,
            commits_skipped: 0,
            issues_imported: 0,
            issues_skipped: 0,
            success: false,
            message,
        }
    }
}

/// 元数据导入器
pub struct MetadataImporter<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> MetadataImporter<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// 从 JSON 文件导入元数据
    pub async fn import_from_file(
        &self,
        file_path: &Path,
        tracking_id: i32,
    ) -> Result<ImportResult> {
        info!("从文件导入元数据: {:?}", file_path);

        // 读取并解析 JSON
        let content = tokio::fs::read_to_string(file_path)
            .await
            .context("读取文件失败")?;

        let metadata: CollectedMetadata =
            serde_json::from_str(&content).context("解析 JSON 失败")?;

        info!(
            "解析完成: platform={}, owner={}, repo={}, commits={}, issues={}",
            metadata.platform,
            metadata.owner,
            metadata.repo,
            metadata.commits.len(),
            metadata.issues.len()
        );

        // 导入数据
        self.import_metadata(&metadata, tracking_id).await
    }

    /// 导入元数据
    pub async fn import_metadata(
        &self,
        metadata: &CollectedMetadata,
        tracking_id: i32,
    ) -> Result<ImportResult> {
        let mut commits_imported = 0;
        let mut commits_skipped = 0;
        let mut issues_imported = 0;
        let mut issues_skipped = 0;

        // 导入 commits
        for commit_info in &metadata.commits {
            match self.import_commit(commit_info, tracking_id).await {
                Ok(true) => commits_imported += 1,
                Ok(false) => commits_skipped += 1,
                Err(e) => {
                    warn!("导入 commit {} 失败: {}", commit_info.sha, e);
                    commits_skipped += 1;
                }
            }
        }

        // 导入 issues
        for issue_info in &metadata.issues {
            match self.import_issue(issue_info, tracking_id).await {
                Ok(true) => issues_imported += 1,
                Ok(false) => issues_skipped += 1,
                Err(e) => {
                    warn!("导入 issue #{} 失败: {}", issue_info.number, e);
                    issues_skipped += 1;
                }
            }
        }

        info!(
            "导入完成: commits={}/{}, issues={}/{}",
            commits_imported,
            commits_imported + commits_skipped,
            issues_imported,
            issues_imported + issues_skipped
        );

        Ok(ImportResult::success(
            commits_imported,
            commits_skipped,
            issues_imported,
            issues_skipped,
        ))
    }

    /// 导入单个 commit
    async fn import_commit(&self, commit_info: &CommitInfo, tracking_id: i32) -> Result<bool> {
        use sea_orm::{ColumnTrait, QueryFilter};

        // 检查是否已存在
        let existing = L1CommitRecords::find()
            .filter(l1_commit_records::Column::TrackingId.eq(tracking_id))
            .filter(l1_commit_records::Column::CommitSha.eq(&commit_info.sha))
            .one(self.db)
            .await
            .context("查询已有 commit 失败")?;

        if existing.is_some() {
            return Ok(false); // 已存在，跳过
        }

        // 插入新记录
        let new_commit = l1_commit_records::ActiveModel {
            tracking_id: Set(tracking_id),
            commit_sha: Set(commit_info.sha.clone()),
            commit_message: Set(commit_info.message.clone()),
            author_name: Set(commit_info.author_name.clone()),
            author_email: Set(commit_info.author_email.clone()),
            committed_at: Set(commit_info.author_date),
            api_url: Set(String::new()), // 空字符串作为默认值
            fetched_at: Set(Utc::now()),
            created_at: Set(Utc::now()),
            ..Default::default()
        };

        L1CommitRecords::insert(new_commit)
            .exec(self.db)
            .await
            .context("插入 commit 失败")?;

        Ok(true)
    }

    /// 导入单个 issue
    async fn import_issue(&self, issue_info: &IssueInfo, tracking_id: i32) -> Result<bool> {
        use sea_orm::{ColumnTrait, QueryFilter};

        let issue_number_str = issue_info.number.to_string();

        // 检查是否已存在
        let existing = Issues::find()
            .filter(issues::Column::TrackingId.eq(tracking_id))
            .filter(issues::Column::IssueNumber.eq(&issue_number_str))
            .one(self.db)
            .await
            .context("查询已有 issue 失败")?;

        if existing.is_some() {
            return Ok(false); // 已存在，跳过
        }

        // 转换 labels
        let labels_json = if let Some(labels) = &issue_info.labels {
            if !labels.is_empty() {
                Some(serde_json::to_value(labels).unwrap_or(serde_json::Value::Null))
            } else {
                None
            }
        } else {
            None
        };

        // 插入新记录
        let new_issue = issues::ActiveModel {
            tracking_id: Set(tracking_id),
            issue_number: Set(issue_number_str),
            title: Set(issue_info.title.clone()),
            state: Set(issue_info.state.clone()),
            author: Set(issue_info
                .author
                .clone()
                .unwrap_or_else(|| "unknown".to_string())),
            api_url: Set(String::new()), // 从 JSON 中无法获取，设为空
            labels: Set(labels_json),
            created_at: Set(issue_info.created_at),
            updated_at: Set(issue_info.updated_at),
            closed_at: Set(issue_info.closed_at),
            raw_payload: Set(None),
            ..Default::default()
        };

        Issues::insert(new_issue)
            .exec(self.db)
            .await
            .context("插入 issue 失败")?;

        Ok(true)
    }

    /// 批量导入多个 JSON 文件
    pub async fn import_batch(
        &self,
        file_paths: Vec<&Path>,
        tracking_id: i32,
    ) -> Result<Vec<ImportResult>> {
        let mut results = Vec::new();

        for file_path in file_paths {
            match self.import_from_file(file_path, tracking_id).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    warn!("导入文件 {:?} 失败: {}", file_path, e);
                    results.push(ImportResult::failed(e.to_string()));
                }
            }
        }

        Ok(results)
    }
}

