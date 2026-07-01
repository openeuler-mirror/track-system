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
            .user_agent("track-system/0.1.0")
            .no_proxy()
            .build()?;

        Ok(Self {
            client,
            token: token.into(),
            base_url: GITEE_API_BASE.to_string(),
        })
    }

    /// 创建带自定义配置的客户端
    pub fn with_config(token: impl Into<String>, timeout: Duration) -> ApiResult<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .user_agent("track-system/0.1.0")
            .no_proxy()
            .build()?;

        Ok(Self {
            client,
            token: token.into(),
            base_url: GITEE_API_BASE.to_string(),
        })
    }

    /// 创建用于测试的客户端（自定义 base_url）
    pub fn for_testing(token: impl Into<String>, base_url: impl Into<String>) -> ApiResult<Self> {
        let client = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .user_agent("track-system/0.1.0")
            .no_proxy()
            .build()?;

        Ok(Self {
            client,
            token: token.into(),
            base_url: base_url.into(),
        })
    }

    /// 创建实现了 Collector trait 的适配器
    pub fn as_collector(self) -> impl Collector {
        use crate::collectors::{adapters::GitClientCollectorAdapter, traits::Platform};
