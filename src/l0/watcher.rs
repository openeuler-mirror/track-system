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

use std::collections::HashSet;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::json;

use crate::collectors::traits::{CollectConfig, Collector, Platform};
use crate::entities::{l0_commits, packages};
use crate::telemetry::Telemetry;

/// L0 仓库拉取摘要
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct L0PollSummary {
    pub commits_inserted: usize,
    pub commits_skipped: usize,
}

/// L0 仓库监听器
pub struct L0Watcher<'a, C>
where
    C: Collector + Send + Sync + ?Sized,
{
    db: &'a DatabaseConnection,
    collector: &'a C,
}

impl<'a, C> L0Watcher<'a, C>
where
    C: Collector + Send + Sync + ?Sized,
{
    pub fn new(db: &'a DatabaseConnection, collector: &'a C) -> Self {
        Self { db, collector }
    }

    /// 轮询指定软件包的 L0 仓库，记录新增 commit
    pub async fn poll_package(
        &self,
        package: &packages::Model,
        branch: &str,
    ) -> Result<L0PollSummary> {
        let repo_url = package
            .l0_repo_url
            .as_deref()
            .ok_or_else(|| anyhow!("package {} missing l0_repo_url", package.name))?;

        let (owner, repo) = parse_github_repo(repo_url)
            .with_context(|| format!("failed to parse GitHub repo from {}", repo_url))?;

        // 构建采集配置
        let config = CollectConfig::new(Platform::GitHub, branch)
            .with_remote(&owner, &repo)
            .with_limit(50);

        // 使用 Collector 采集 commits
        let result = self
            .collector
            .collect(&config)
            .await
            .context("failed to collect commits from GitHub")?;

        let commits = result.commits;

        if commits.is_empty() {
            return Ok(L0PollSummary::default());
        }

        let mut existing: HashSet<String> = l0_commits::Entity::find()
            .filter(l0_commits::Column::PackageId.eq(package.id))
            .all(self.db)
            .await?
            .into_iter()
            .map(|model| model.commit_sha)
            .collect();

        let mut summary = L0PollSummary::default();

        for commit in commits {
            if !existing.insert(commit.sha.clone()) {
                summary.commits_skipped += 1;
                continue;
            }

            let metadata = json!({
                "author": commit.author,
                "email": commit.email,
                "files_changed": commit.files_changed,
            });

            let active = l0_commits::ActiveModel {
                package_id: Set(package.id),
                repo: Set(format!("{}/{}", owner, repo)),
                commit_sha: Set(commit.sha.clone()),
                summary: Set(commit.message.clone()),
                authored_at: Set(commit.date),
                metadata: Set(Some(metadata)),
                created_at: Set(Utc::now()),
                updated_at: Set(Utc::now()),
                ..Default::default()
            };

            active.insert(self.db).await?;
            summary.commits_inserted += 1;
        }

        Telemetry::l0_poll_summary(
            package.id,
            summary.commits_inserted,
            summary.commits_skipped,
        );
        Ok(summary)
    }
}

fn parse_github_repo(url: &str) -> Result<(String, String)> {
    let trimmed = url.trim().trim_end_matches('/').trim_end_matches(".git");

    let path = if let Some(idx) = trimmed.find("github.com/") {
        &trimmed[idx + "github.com/".len()..]
    } else {
        trimmed
    };

    let mut segments = path.split('/').filter(|segment| !segment.is_empty());
    let owner = segments
        .next()
        .ok_or_else(|| anyhow!("invalid repository url: {}", url))?;
    let repo = segments
        .next()
        .ok_or_else(|| anyhow!("invalid repository url: {}", url))?;

    Ok((owner.to_string(), repo.to_string()))
}

