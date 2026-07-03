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
