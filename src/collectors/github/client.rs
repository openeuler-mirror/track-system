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

        loop {
            let mut request = self.client.get(url);

            if let Some(token) = &self.token {
                request = request.bearer_auth(token);
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
            } else if body.is_empty() {
                format!("HTTP {}", status.as_u16())
            } else {
                body
            };

            let error = ApiError::from_status(status.as_u16(), message);

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
impl GitClient for GitHubClient {
    async fn get_repository(&self, owner: &str, repo: &str) -> ApiResult<Repository> {
        let url = format!("{}/repos/{}/{}", self.base_url, owner, repo);
        let repository: GitHubRepository = self.get(&url).await?;
        Ok(repository.into())
    }

    async fn get_branches(&self, owner: &str, repo: &str) -> ApiResult<Vec<Branch>> {
        let url = format!("{}/repos/{}/{}/branches", self.base_url, owner, repo);
        let branches: Vec<GitHubBranch> = self.get(&url).await?;
        Ok(branches.into_iter().map(Into::into).collect())
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

        let commits: Vec<GitHubCommit> = self.get(&url).await?;
        Ok(commits.into_iter().map(Into::into).collect())
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
        let file: GitHubFileContent = self.get(&url).await?;
        Ok(file.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::traits::CommitsParams;
    use httpmock::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_github_client_new() {
        let client = GitHubClient::new("token").unwrap();
        assert_eq!(client.token, Some("token".to_string()));
        assert_eq!(client.base_url, GITHUB_API_BASE);

        let client_no_token = GitHubClient::new("").unwrap();
        assert_eq!(client_no_token.token, None);
    }

    #[tokio::test]
    async fn test_github_client_as_collector() {
        let client = GitHubClient::new("token").unwrap();
        let _collector = client.as_collector();
    }

    #[tokio::test]
    async fn test_get_repository() {
        let server = MockServer::start();
        let client = GitHubClient::for_testing("token", server.base_url()).unwrap();

        let repo_response = json!({
            "id": 1,
            "name": "test-repo",
            "full_name": "owner/test-repo",
            "html_url": "http://localhost/owner/test-repo",
            "description": "test repo",
            "private": false,
            "created_at": "2023-01-01T00:00:00Z",
            "updated_at": "2023-01-01T00:00:00Z",
            "default_branch": "main",
            "clone_url": "http://localhost/owner/test-repo.git"
        });

        let _mock = server.mock(|when, then| {
            when.method(GET)
                .path("/repos/owner/test-repo")
                .header("Authorization", "Bearer token");
            then.status(200).json_body(repo_response);
