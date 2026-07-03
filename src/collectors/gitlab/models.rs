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

//! GitLab API 响应模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::collectors::traits::{Branch, Commit, CommitStats, FileContent, Repository};

/// GitLab 仓库信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLabRepository {
    pub id: i64,
    pub name: String,
    pub path_with_namespace: String,
    pub description: Option<String>,
    pub web_url: String,
    pub default_branch: String,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
}

impl From<GitLabRepository> for Repository {
    fn from(repo: GitLabRepository) -> Self {
        Self {
            id: repo.id,
            name: repo.name.clone(),
            full_name: repo.path_with_namespace,
            description: repo.description,
            html_url: repo.web_url,
            default_branch: repo.default_branch,
            created_at: repo.created_at,
            updated_at: repo.last_activity_at,
        }
    }
}

/// GitLab 分支信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLabBranch {
    pub name: String,
    pub commit: GitLabBranchCommit,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLabBranchCommit {
    pub id: String,
}

impl From<GitLabBranch> for Branch {
    fn from(branch: GitLabBranch) -> Self {
        Self {
            name: branch.name,
            commit_sha: branch.commit.id,
            protected: branch.protected,
        }
    }
}

/// GitLab Commit 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLabCommit {
    pub id: String,
    pub short_id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub authored_date: DateTime<Utc>,
    pub committer_name: String,
    pub committer_email: String,
    pub committed_date: DateTime<Utc>,
    pub web_url: String,
    #[serde(default)]
    pub stats: Option<GitLabCommitStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLabCommitStats {
    pub additions: u32,
    pub deletions: u32,
    pub total: u32,
}

impl From<GitLabCommit> for Commit {
    fn from(commit: GitLabCommit) -> Self {
        let title = commit
            .title
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_else(|| {
                commit
                    .message
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            });

        Self {
            sha: commit.id,
            title,
            message: commit.message,
            author_name: commit.author_name,
            author_email: commit.author_email,
            author_date: commit.authored_date,
            committer_name: commit.committer_name,
            committer_email: commit.committer_email,
            committer_date: commit.committed_date,
            html_url: commit.web_url,
            stats: commit.stats.map(|s| CommitStats {
                additions: s.additions,
                deletions: s.deletions,
                total: s.total,
            }),
        }
    }
}

/// GitLab 文件内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLabFileContent {
    pub file_name: String,
    pub file_path: String,
    pub size: u64,
    pub encoding: String,
    pub content: String,
    pub content_sha256: String,
    pub ref_name: String,
    pub blob_id: String,
}

impl From<GitLabFileContent> for FileContent {
    fn from(file: GitLabFileContent) -> Self {
        Self {
            name: file.file_name,
            path: file.file_path.clone(),
            sha: file.blob_id,
            size: file.size,
            content: file.content,
            encoding: file.encoding,
            download_url: String::new(), // GitLab 不直接提供下载 URL
        }
    }
}
