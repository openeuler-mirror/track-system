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

use crate::collectors::traits::{Branch, Commit, CommitStats, FileContent, Repository};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaRepository {
    pub id: i64,
    pub full_name: String,
    pub description: Option<String>,
    pub html_url: Option<String>,
    pub default_branch: Option<String>,
    pub owner: Owner,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Owner {
    pub login: String,
}

impl From<GiteaRepository> for Repository {
    fn from(repo: GiteaRepository) -> Self {
        Repository {
            id: repo.id,
            name: repo.name.clone(),
            full_name: repo.full_name,
            description: repo.description,
            html_url: repo.html_url.unwrap_or_default(),
            default_branch: repo.default_branch.unwrap_or_else(|| "master".to_string()),
            created_at: repo.created_at,
            updated_at: repo.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaBranch {
    pub name: String,
    pub commit: GiteaBranchCommit,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaBranchCommit {
    pub id: String,
}

impl From<GiteaBranch> for Branch {
    fn from(branch: GiteaBranch) -> Self {
        Branch {
            name: branch.name,
            commit_sha: branch.commit.id,
            protected: branch.protected,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaCommit {
    pub sha: String,
    pub commit: GiteaCommitMetadata,
    pub html_url: Option<String>,
    pub stats: Option<GiteaCommitStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaCommitMetadata {
    #[serde(default)]
    pub title: Option<String>,
    pub message: String,
    pub author: GiteaCommitAuthor,
    pub committer: Option<GiteaCommitAuthor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaCommitAuthor {
    pub name: Option<String>,
    pub email: Option<String>,
    pub date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaCommitStats {
    pub additions: u32,
    pub deletions: u32,
    pub total: u32,
}

impl From<GiteaCommit> for Commit {
    fn from(commit: GiteaCommit) -> Self {
        let author = &commit.commit.author;
        let committer = commit.commit.committer.as_ref().unwrap_or(author);
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
            title,
            message: commit.commit.message,
            author_name: author.name.clone().unwrap_or_default(),
            author_email: author.email.clone().unwrap_or_default(),
            author_date: author.date,
            committer_name: committer.name.clone().unwrap_or_default(),
            committer_email: committer.email.clone().unwrap_or_default(),
            committer_date: committer.date,
            html_url: commit.html_url.unwrap_or_default(),
            stats: commit.stats.map(|stats| CommitStats {
                additions: stats.additions,
                deletions: stats.deletions,
                total: stats.total,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaFileContent {
    pub name: String,
    pub path: String,
    pub sha: String,
    pub size: u64,
    pub encoding: String,
    pub content: String,
    pub html_url: Option<String>,
}

impl From<GiteaFileContent> for FileContent {
    fn from(file: GiteaFileContent) -> Self {
        FileContent {
            name: file.name,
            path: file.path,
            sha: file.sha,
            size: file.size,
            content: file.content,
            encoding: file.encoding,
            download_url: file.html_url.unwrap_or_default(),
        }
    }
}
