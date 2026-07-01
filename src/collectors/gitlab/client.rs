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
            token: token_opt,
            base_url: base_url.into(),
        })
    }

    /// 创建实现了 Collector trait 的适配器
    pub fn as_collector(self) -> impl Collector {
        use crate::collectors::{adapters::GitClientCollectorAdapter, traits::Platform};
        GitClientCollectorAdapter::new(self, Platform::GitLab)
    }

    /// URL 编码项目路径
    fn encode_project_path(&self, owner: &str, repo: &str) -> String {
        let path = format!("{}/{}", owner, repo);
        urlencoding::encode(&path).to_string()
    }

    /// 发送 GET 请求
    async fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> ApiResult<T> {
        let mut retries = 0;

        loop {
            let mut request = self.client.get(url);

            // GitLab 使用 PRIVATE-TOKEN 头或 access_token 参数
            if let Some(token) = &self.token {
                request = request.header("PRIVATE-TOKEN", token);
            }

            let response = request.send().await?;
            let status = response.status();

            if status.is_success() {
                return response.json::<T>().await.map_err(ApiError::from);
            }

            let body = response.text().await.unwrap_or_default();
            let message = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                json.get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or(body.as_str())
                    .to_string()
            } else {
