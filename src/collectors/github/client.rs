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

use crate::collectors::{
    error::{ApiError, ApiResult},
    traits::{Branch, Collector, Commit, CommitsParams, FileContent, GitClient, Repository},
};

use super::models::{GitHubBranch, GitHubCommit, GitHubFileContent, GitHubRepository};

const GITHUB_API_BASE: &str = "https://api.github.com";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u32 = 3;

/// GitHub API 客户端
pub struct GitHubClient {
    client: Client,
    token: Option<String>,
    base_url: String,
}

impl GitHubClient {
    /// 创建默认客户端
    pub fn new(token: impl Into<String>) -> ApiResult<Self> {
        Self::with_config(token, DEFAULT_TIMEOUT)
    }

    /// 创建带自定义超时的客户端
    pub fn with_config(token: impl Into<String>, timeout: Duration) -> ApiResult<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .user_agent("track-system/0.1.0")
            .no_proxy()
            .build()?;

        let token_str = token.into();
        let token_opt = if token_str.is_empty() {
            None
        } else {
            Some(token_str)
        };

        Ok(Self {
            client,
            token: token_opt,
            base_url: GITHUB_API_BASE.to_string(),
        })
    }

    /// 创建实现了 Collector trait 的适配器
    pub fn as_collector(self) -> impl Collector {
        use crate::collectors::{adapters::GitClientCollectorAdapter, traits::Platform};
        GitClientCollectorAdapter::new(self, Platform::GitHub)
    }

    /// 创建用于测试的客户端
    pub fn for_testing(token: impl Into<String>, base_url: impl Into<String>) -> ApiResult<Self> {
        let client = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .user_agent("track-system/0.1.0-test")
            .no_proxy()
            .build()?;

        let token_str = token.into();
        let token_opt = if token_str.is_empty() {
            None
        } else {
            Some(token_str)
        };

        Ok(Self {
            client,
            token: token_opt,
            base_url: base_url.into(),
        })
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> ApiResult<T> {
        let mut retries = 0;

