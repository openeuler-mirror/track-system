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
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::collectors::traits::{
    Branch, Commit, CommitStats, FileContent, Issue, IssueState, Repository,
};

/// Gitee 仓库响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeRepository {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub html_url: String,
    pub default_branch: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<GiteeRepository> for Repository {
    fn from(repo: GiteeRepository) -> Self {
        Self {
            id: repo.id,
            name: repo.name,
            full_name: repo.full_name,
            description: repo.description,
            html_url: repo.html_url,
            default_branch: repo.default_branch,
            created_at: repo
                .created_at
                .parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now()),
            updated_at: repo
                .updated_at
                .parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now()),
        }
    }
}

/// Gitee 分支响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeBranch {
    pub name: String,
    pub commit: GiteeCommitRef,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeCommitRef {
    pub sha: String,
}

impl From<GiteeBranch> for Branch {
    fn from(branch: GiteeBranch) -> Self {
        Self {
            name: branch.name,
            commit_sha: branch.commit.sha,
            protected: branch.protected,
        }
    }
}

/// Gitee Commit 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeCommit {
    pub sha: String,
    pub commit: GiteeCommitDetail,
    pub html_url: String,
    #[serde(default)]
    pub stats: Option<GiteeCommitStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeCommitDetail {
    #[serde(default)]
    pub title: Option<String>,
    pub message: String,
    pub author: GiteeUser,
    pub committer: GiteeUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeUser {
    pub name: String,
    pub email: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeCommitStats {
    pub additions: u32,
    pub deletions: u32,
    pub total: u32,
}

impl From<GiteeCommit> for Commit {
    fn from(commit: GiteeCommit) -> Self {
        let derived_title = commit
            .commit
            .title
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                commit
                    .commit
                    .message
                    .lines()
                    .next()
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default()
            });

        Self {
            sha: commit.sha,
            title: derived_title,
            message: commit.commit.message,
            author_name: commit.commit.author.name,
            author_email: commit.commit.author.email,
            author_date: commit
                .commit
                .author
                .date
                .parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now()),
            committer_name: commit.commit.committer.name,
            committer_email: commit.commit.committer.email,
            committer_date: commit
                .commit
                .committer
                .date
                .parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now()),
