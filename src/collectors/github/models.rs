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
