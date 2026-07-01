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

//! GitLab API 客户端实现

use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;

use crate::collectors::{
    error::{ApiError, ApiResult},
    traits::{Branch, Collector, Commit, CommitsParams, FileContent, GitClient, Repository},
};

use super::models::{GitLabBranch, GitLabCommit, GitLabFileContent, GitLabRepository};

const GITLAB_API_BASE: &str = "https://gitlab.com/api/v4";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u32 = 3;

/// GitLab API 客户端
pub struct GitLabClient {
    client: Client,
    token: Option<String>,
    base_url: String,
}

impl GitLabClient {
    /// 创建默认客户端（使用 gitlab.com）
    pub fn new(token: impl Into<String>) -> ApiResult<Self> {
        Self::with_base_url(GITLAB_API_BASE, token)
    }

    /// 创建自定义 GitLab 实例的客户端
    pub fn with_base_url(base_url: impl Into<String>, token: impl Into<String>) -> ApiResult<Self> {
        let client = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
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
            base_url: base_url.into(),
        })
    }

    /// 创建带自定义超时的客户端
    pub fn with_config(
        base_url: impl Into<String>,
        token: impl Into<String>,
        timeout: Duration,
    ) -> ApiResult<Self> {
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
