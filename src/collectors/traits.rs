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

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

use super::error::ApiResult;

/// 仓库信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub html_url: String,
    pub default_branch: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 分支信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub commit_sha: String,
    pub protected: bool,
}

/// Commit 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub title: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub author_date: DateTime<Utc>,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_date: DateTime<Utc>,
    pub html_url: String,
    pub stats: Option<CommitStats>,
}

/// Commit 统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitStats {
    pub additions: u32,
    pub deletions: u32,
    pub total: u32,
}

/// 文件内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    pub name: String,
    pub path: String,
    pub sha: String,
    pub size: u64,
    pub content: String,  // Base64 编码的内容
    pub encoding: String, // "base64"
    pub download_url: String,
}

/// 分页参数
#[derive(Debug, Clone)]
pub struct PaginationParams {
    pub page: u32,
    pub per_page: u32,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 30,
        }
    }
}

/// Issue 状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IssueState {
    Open,
    Closed,
    All,
}

impl IssueState {
    pub fn as_query_value(&self) -> &'static str {
        match self {
            IssueState::Open => "open",
            IssueState::Closed => "closed",
            IssueState::All => "all",
        }
    }

    pub fn parse_str(value: &str) -> Self {
        match value {
            "open" => IssueState::Open,
            "closed" => IssueState::Closed,
            _ => IssueState::All,
        }
    }
}

impl fmt::Display for IssueState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_query_value())
    }
}

/// Issue 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub number: i64,
    pub title: String,
    pub state: IssueState,
    pub author: String,
    pub api_url: String,
    pub labels: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub raw_payload: Value,
}

/// Issue 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueEvent {
    pub event_type: String,
    pub actor: Option<String>,
    pub occurred_at: DateTime<Utc>,
    pub raw_payload: Value,
}

/// Commits 查询参数
#[derive(Debug, Clone)]
pub struct CommitsParams {
    pub branch: String,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub page: u32,
    pub per_page: u32,
}

impl CommitsParams {
    pub fn new(branch: impl Into<String>) -> Self {
        Self {
            branch: branch.into(),
            since: None,
            until: None,
            page: 1,
            per_page: 30,
        }
    }

    pub fn since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    pub fn until(mut self, until: DateTime<Utc>) -> Self {
        self.until = Some(until);
        self
    }

    pub fn page(mut self, page: u32) -> Self {
        self.page = page;
        self
    }

    pub fn per_page(mut self, per_page: u32) -> Self {
        self.per_page = per_page;
        self
    }
}

/// Git 平台客户端通用接口
#[async_trait]
pub trait GitClient: Send + Sync {
    /// 获取仓库信息
    async fn get_repository(&self, owner: &str, repo: &str) -> ApiResult<Repository>;

    /// 获取分支列表
    async fn get_branches(&self, owner: &str, repo: &str) -> ApiResult<Vec<Branch>>;

    /// 获取指定分支的 commits
    async fn get_commits(
        &self,
        owner: &str,
        repo: &str,
        params: CommitsParams,
    ) -> ApiResult<Vec<Commit>>;

    /// 获取文件内容
    async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        branch: &str,
    ) -> ApiResult<FileContent>;

    /// 解码 Base64 内容
    fn decode_content(&self, content: &str) -> ApiResult<String> {
        use base64::{engine::general_purpose, Engine as _};

        // 移除换行符
        let cleaned = content.replace('\n', "");

        general_purpose::STANDARD
            .decode(cleaned.as_bytes())
            .map_err(|e| super::error::ApiError::Base64Error(e.to_string()))
            .and_then(|bytes| {
                String::from_utf8(bytes)
                    .map_err(|e| super::error::ApiError::Base64Error(e.to_string()))
            })
    }
}

/// Issues 客户端扩展接口
#[async_trait]
pub trait IssueClient: Send + Sync {
    async fn get_issues(
        &self,
        owner: &str,
        repo: &str,
        params: IssueParams,
    ) -> ApiResult<Vec<Issue>>;

    async fn get_issue_events(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: i64,
    ) -> ApiResult<Vec<IssueEvent>> {
        Ok(Vec::new())
    }
}

/// Issues 查询参数
#[derive(Debug, Clone)]
pub struct IssueParams {
    pub state: IssueState,
    pub page: u32,
    pub per_page: u32,
