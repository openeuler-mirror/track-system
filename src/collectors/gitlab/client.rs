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
                body
            };

            // 处理错误
            let error = ApiError::from_status(status.as_u16(), message.clone());

            // 判断是否需要重试
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
impl GitClient for GitLabClient {
    async fn get_repository(&self, owner: &str, repo: &str) -> ApiResult<Repository> {
        let project_path = self.encode_project_path(owner, repo);
        let url = format!("{}/projects/{}", self.base_url, project_path);

        let gitlab_repo: GitLabRepository = self.get(&url).await?;
        Ok(gitlab_repo.into())
    }

    async fn get_branches(&self, owner: &str, repo: &str) -> ApiResult<Vec<Branch>> {
        let project_path = self.encode_project_path(owner, repo);
        let url = format!(
            "{}/projects/{}/repository/branches",
            self.base_url, project_path
        );

        let gitlab_branches: Vec<GitLabBranch> = self.get(&url).await?;
        Ok(gitlab_branches.into_iter().map(|b| b.into()).collect())
    }

    async fn get_commits(
        &self,
        owner: &str,
        repo: &str,
        params: CommitsParams,
    ) -> ApiResult<Vec<Commit>> {
        let project_path = self.encode_project_path(owner, repo);
        let mut url = format!(
            "{}/projects/{}/repository/commits?ref_name={}&per_page={}&page={}",
            self.base_url, project_path, params.branch, params.per_page, params.page
        );

        // 添加时间范围参数
        if let Some(since) = params.since {
            url.push_str(&format!("&since={}", since.to_rfc3339()));
        }
        if let Some(until) = params.until {
            url.push_str(&format!("&until={}", until.to_rfc3339()));
        }

        // 添加 with_stats 参数以获取统计信息
        url.push_str("&with_stats=true");

        let gitlab_commits: Vec<GitLabCommit> = self.get(&url).await?;
        Ok(gitlab_commits.into_iter().map(|c| c.into()).collect())
    }

    async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        branch: &str,
    ) -> ApiResult<FileContent> {
        let project_path = self.encode_project_path(owner, repo);
        let file_path = urlencoding::encode(path);
        let url = format!(
            "{}/projects/{}/repository/files/{}?ref={}",
            self.base_url, project_path, file_path, branch
        );

        let gitlab_file: GitLabFileContent = self.get(&url).await?;
        Ok(gitlab_file.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::traits::CommitsParams;
    use httpmock::prelude::*;
    use serde_json::json;

    #[test]
    fn test_encode_project_path() {
        let client = GitLabClient::new("test-token").unwrap();
        let encoded = client.encode_project_path("gitlab-org", "gitlab");
        assert_eq!(encoded, "gitlab-org%2Fgitlab");
    }

    #[test]
    fn test_client_creation() {
        let client = GitLabClient::new("test-token");
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.base_url, GITLAB_API_BASE);
        assert!(client.token.is_some());
    }

    #[test]
    fn test_custom_base_url() {
        let client = GitLabClient::with_base_url("https://gitlab.example.com/api/v4", "test-token");
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.base_url, "https://gitlab.example.com/api/v4");
    }

    #[tokio::test]
    async fn test_gitlab_client_as_collector() {
        let client = GitLabClient::new("token").unwrap();
        let _collector = client.as_collector();
    }

    #[tokio::test]
    async fn test_get_repository() {
        let server = MockServer::start();
        let client = GitLabClient::with_base_url(server.base_url(), "token").unwrap();

        let repo_response = json!({
            "id": 1,
            "name": "test-repo",
            "path_with_namespace": "owner/test-repo",
            "web_url": "http://localhost/owner/test-repo",
            "description": "test repo",
            "visibility": "public",
            "created_at": "2023-01-01T00:00:00Z",
            "last_activity_at": "2023-01-01T00:00:00Z",
            "default_branch": "main",
            "http_url_to_repo": "http://localhost/owner/test-repo.git"
        });

        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/projects/owner%2Ftest-repo")
                .header("PRIVATE-TOKEN", "token");
            then.status(200).json_body(repo_response);
        });

        let result = client.get_repository("owner", "test-repo").await;
        mock.assert();
        assert!(result.is_ok());
        let repo = result.unwrap();
        assert_eq!(repo.name, "test-repo");
    }

    #[tokio::test]
    async fn test_get_branches() {
        let server = MockServer::start();
        let client = GitLabClient::with_base_url(server.base_url(), "token").unwrap();

        let branch_response = json!([
            {
                "name": "main",
                "commit": {
                    "id": "sha",
                    "web_url": "url"
                },
                "protected": true
            }
        ]);

        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/projects/owner%2Ftest-repo/repository/branches")
                .header("PRIVATE-TOKEN", "token");
            then.status(200).json_body(branch_response);
        });

        let result = client.get_branches("owner", "test-repo").await;
        mock.assert();
        assert!(result.is_ok());
        let branches = result.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].name, "main");
    }

    #[tokio::test]
    async fn test_get_commits() {
        let server = MockServer::start();
        let client = GitLabClient::with_base_url(server.base_url(), "token").unwrap();

        let commits_response = json!([
            {
                "id": "sha",
                "short_id": "short_sha",
                "message": "message",
                "author_name": "author",
                "author_email": "email",
                "authored_date": "2023-01-01T00:00:00Z",
                "committer_name": "committer",
                "committer_email": "email",
                "committed_date": "2023-01-01T00:00:00Z",
