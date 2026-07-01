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
        GitClientCollectorAdapter::new(self, Platform::Gitee)
    }

    /// 执行 GET 请求（带重试）
    async fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> ApiResult<T> {
        let mut retries = 0;

        loop {
            let response = self
                .client
                .get(url)
                .query(&[("access_token", &self.token)])
                .send()
                .await?;

            let status = response.status();

            info!("Gitee API GET {}: {}", url, status);
            // 成功响应
            if status.is_success() {
                return response.json::<T>().await.map_err(ApiError::from);
            }

            // 错误响应
            let error_body = response.text().await.unwrap_or_default();

            // 解析错误消息
            let error_message = serde_json::from_str::<GiteeError>(&error_body)
                .ok()
                .map(|e| e.message)
                .unwrap_or_else(|| {
                    if error_body.is_empty() {
                        format!("HTTP {}", status.as_u16())
                    } else {
                        error_body
                    }
                });

            let error = ApiError::from_status(status.as_u16(), error_message);

            // 判断是否可重试
            if error.is_retryable() && retries < MAX_RETRIES {
                retries += 1;
                tokio::time::sleep(Duration::from_secs(2u64.pow(retries))).await;
                continue;
            }

            return Err(error);
        }
    }
}

#[async_trait]
impl GitClient for GiteeClient {
    async fn get_repository(&self, owner: &str, repo: &str) -> ApiResult<Repository> {
        let url = format!("{}/repos/{}/{}", self.base_url, owner, repo);
        let gitee_repo: GiteeRepository = self.get(&url).await?;
        Ok(gitee_repo.into())
    }

    async fn get_branches(&self, owner: &str, repo: &str) -> ApiResult<Vec<Branch>> {
        let url = format!("{}/repos/{}/{}/branches", self.base_url, owner, repo);
        let gitee_branches: Vec<GiteeBranch> = self.get(&url).await?;
        Ok(gitee_branches.into_iter().map(Into::into).collect())
    }

    async fn get_commits(
        &self,
        owner: &str,
        repo: &str,
        params: CommitsParams,
    ) -> ApiResult<Vec<Commit>> {
        let mut url = format!(
            "{}/repos/{}/{}/commits?sha={}&page={}&per_page={}",
            self.base_url, owner, repo, params.branch, params.page, params.per_page
        );

        if let Some(since) = params.since {
            url.push_str(&format!("&since={}", since.to_rfc3339()));
        }

        if let Some(until) = params.until {
            url.push_str(&format!("&until={}", until.to_rfc3339()));
        }

        let gitee_commits: Vec<GiteeCommit> = self.get(&url).await?;
        Ok(gitee_commits.into_iter().map(Into::into).collect())
    }

    async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        branch: &str,
    ) -> ApiResult<FileContent> {
        let url = format!(
            "{}/repos/{}/{}/contents/{}?ref={}",
            self.base_url, owner, repo, path, branch
        );
        let gitee_file: GiteeFileContent = self.get(&url).await?;
        Ok(gitee_file.into())
    }
}

#[async_trait]
impl IssueClient for GiteeClient {
    async fn get_issues(
        &self,
        owner: &str,
        repo: &str,
        params: IssueParams,
    ) -> ApiResult<Vec<Issue>> {
        let mut url = format!(
            "{}/repos/{}/{}/issues?page={}&per_page={}&state={}",
            self.base_url,
            owner,
            repo,
            params.page,
            params.per_page,
            params.state.as_query_value()
        );

        if let Some(since) = params.since {
            url.push_str(&format!("&since={}", since.to_rfc3339()));
        }

        let issues: Vec<GiteeIssue> = self.get(&url).await?;
        Ok(issues.into_iter().map(Into::into).collect())
    }
}

