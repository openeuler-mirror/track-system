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
use serde::Deserialize;

use crate::collectors::traits::{Branch, Commit, CommitStats, FileContent, Repository};

#[derive(Debug, Deserialize)]
pub struct GitHubRepository {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub html_url: String,
    #[serde(default)]
    pub default_branch: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<GitHubRepository> for Repository {
    fn from(repo: GitHubRepository) -> Self {
        Repository {
            id: repo.id,
            name: repo.name,
            full_name: repo.full_name,
            description: repo.description,
            html_url: repo.html_url,
            default_branch: if repo.default_branch.is_empty() {
                "main".to_string()
            } else {
                repo.default_branch
            },
            created_at: repo.created_at,
            updated_at: repo.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GitHubBranch {
    pub name: String,
    #[serde(default)]
    pub protected: bool,
    pub commit: GitHubBranchCommit,
}

#[derive(Debug, Deserialize)]
pub struct GitHubBranchCommit {
    pub sha: String,
}

impl From<GitHubBranch> for Branch {
    fn from(branch: GitHubBranch) -> Self {
        Branch {
            name: branch.name,
            commit_sha: branch.commit.sha,
            protected: branch.protected,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GitHubCommit {
    pub sha: String,
    pub html_url: String,
    pub commit: GitHubCommitInfo,
    #[serde(default)]
    pub stats: Option<GitHubCommitStats>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubCommitInfo {
    #[serde(default)]
    pub title: Option<String>,
    pub message: String,
    pub author: Option<GitHubCommitAuthor>,
    pub committer: Option<GitHubCommitAuthor>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubCommitAuthor {
    pub name: Option<String>,
    pub email: Option<String>,
    pub date: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubCommitStats {
    pub additions: u32,
    pub deletions: u32,
    pub total: u32,
}

impl From<GitHubCommitStats> for CommitStats {
    fn from(stats: GitHubCommitStats) -> Self {
        CommitStats {
            additions: stats.additions,
            deletions: stats.deletions,
            total: stats.total,
        }
    }
}

impl From<GitHubCommit> for Commit {
    fn from(commit: GitHubCommit) -> Self {
        let default_author = GitHubCommitAuthor {
            name: Some("unknown".to_string()),
            email: Some("unknown".to_string()),
            date: Utc::now(),
        };

        let author = commit.commit.author.unwrap_or(default_author);
        let author_name = author.name.unwrap_or_else(|| "unknown".to_string());
        let author_email = author.email.unwrap_or_else(|| "unknown".to_string());
        let author_date = author.date;

        let (committer_name, committer_email, committer_date) = match commit.commit.committer {
            Some(committer) => (
                committer.name.unwrap_or_else(|| "unknown".to_string()),
                committer.email.unwrap_or_else(|| "unknown".to_string()),
                committer.date,
            ),
            None => (author_name.clone(), author_email.clone(), author_date),
        };

        let title = commit
            .commit
            .title
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_else(|| {
                commit
                    .commit
                    .message
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            });

        Commit {
            sha: commit.sha,
