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

//! L0仓库轮询服务
//!
//! 负责定期从L0（上游社区）仓库轮询新commit并检测差异

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tracing::{debug, info};

use crate::collectors::traits::{CollectConfig, Collector, Platform};
use crate::entities::l0_commits;

/// L0轮询摘要
#[derive(Debug, Clone)]
pub struct L0PollingResult {
    /// 拉取时间
    pub polled_at: chrono::DateTime<chrono::Utc>,
    /// 新发现的commit数
    pub new_commits: usize,
    /// 与L1的差异commit数
    pub diff_commits: usize,
}

impl L0PollingResult {
    pub fn new() -> Self {
        Self {
            polled_at: Utc::now(),
            new_commits: 0,
            diff_commits: 0,
        }
    }
}

impl Default for L0PollingResult {
    fn default() -> Self {
        Self::new()
    }
}

/// L0仓库轮询服务
pub struct L0PollingService<'a, C>
where
    C: Collector + Send + Sync,
{
    db: &'a DatabaseConnection,
    collector: &'a C,
}

impl<'a, C> L0PollingService<'a, C>
where
    C: Collector + Send + Sync,
{
    pub fn new(db: &'a DatabaseConnection, collector: &'a C) -> Self {
        Self { db, collector }
    }

    /// 轮询L0仓库
    pub async fn poll_l0(
        &self,
        package_id: i32,
        owner: &str,
        repo: &str,
        branch: &str,
        platform: Platform,
    ) -> Result<L0PollingResult> {
        let mut result = L0PollingResult::new();

        // 构建采集配置
        let config = CollectConfig::new(platform, branch)
            .with_remote(owner, repo)
            .with_limit(100);

        // 使用 Collector 采集 commits
        let collect_result = self
            .collector
            .collect(&config)
            .await
            .context("采集L0 commits失败")?;

        let commits = collect_result.commits;
        let total_checked = commits.len();

        info!(
            package_id = package_id,
            commits_count = total_checked,
            "采集到 {} 个 commits",
            total_checked
        );

        let mut total_new = 0;

        for commit in &commits {
            // 检查该commit是否已存在
            let existing = l0_commits::Entity::find()
                .filter(l0_commits::Column::PackageId.eq(package_id))
                .filter(l0_commits::Column::CommitSha.eq(&commit.sha))
                .one(self.db)
                .await?;

            if existing.is_none() {
                // 新的commit，记录到数据库
                let now = Utc::now();
                let l0_commit = l0_commits::ActiveModel {
                    package_id: Set(package_id),
                    repo: Set(format!("{}/{}", owner, repo)),
                    commit_sha: Set(commit.sha.clone()),
                    summary: Set(commit.message.clone()),
                    authored_at: Set(commit.date),
                    metadata: Set(Some(serde_json::json!({
                        "author_name": commit.author,
                        "author_email": commit.email,
                        "files_changed": commit.files_changed,
                    }))),
                    created_at: Set(now),
                    updated_at: Set(now),
