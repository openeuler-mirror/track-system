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
use reqwest::Client;
use std::time::Duration;
use tracing::info;

use crate::collectors::{
    error::{ApiError, ApiResult},
    traits::{
        Branch, Collector, Commit, CommitsParams, FileContent, GitClient, Issue, IssueClient,
        IssueParams, Repository,
    },
};

use super::models::{
    GiteeBranch, GiteeCommit, GiteeError, GiteeFileContent, GiteeIssue, GiteeRepository,
};

const GITEE_API_BASE: &str = "https://gitee.com/api/v5";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u32 = 3;

/// Gitee API 客户端
pub struct GiteeClient {
    client: Client,
    token: String,
    base_url: String,
}

impl GiteeClient {
    /// 创建新的 Gitee 客户端
    pub fn new(token: impl Into<String>) -> ApiResult<Self> {
        let client = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
