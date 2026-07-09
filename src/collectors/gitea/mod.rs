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
    traits::{Branch, Collector, Commit, CommitsParams, FileContent, GitClient, Repository},
};

use self::models::{GiteaBranch, GiteaCommit, GiteaFileContent, GiteaRepository};

mod models;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u32 = 3;

/// Gitea API 客户端
pub struct GiteaClient {
    client: Client,
    token: String,
    base_url: String,
}

impl GiteaClient {
    pub fn new(token: impl Into<String>, base_url: impl Into<String>) -> ApiResult<Self> {
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
        GitClientCollectorAdapter::new(self, Platform::Gitea)
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> ApiResult<T> {
        let url = format!("{}{}", self.base_url, path);
        let mut retries = 0;

        loop {
            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("token {}", self.token))
                .send()
                .await?;

            let status = response.status();

            if status.is_success() {
                return response.json::<T>().await.map_err(ApiError::from);
            }

            let body = response.text().await.unwrap_or_default();
            let error = ApiError::from_status(status.as_u16(), body);

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
impl GitClient for GiteaClient {
    async fn get_repository(&self, owner: &str, repo: &str) -> ApiResult<Repository> {
        let path = format!("/repos/{}/{}", owner, repo);
        let repository: GiteaRepository = self.get(&path).await?;
        Ok(repository.into())
    }

    async fn get_branches(&self, owner: &str, repo: &str) -> ApiResult<Vec<Branch>> {
        let path = format!("/repos/{}/{}/branches", owner, repo);
        let branches: Vec<GiteaBranch> = self.get(&path).await?;
        Ok(branches.into_iter().map(Into::into).collect())
    }

    async fn get_commits(
        &self,
        owner: &str,
        repo: &str,
        params: CommitsParams,
    ) -> ApiResult<Vec<Commit>> {
        info!(
            "get_commits: base_url={}, token={}, owner={}, repo={}, params={:?}",
            self.base_url, self.token, owner, repo, params
        );
        let mut path = format!(
            "/repos/{}/{}/commits?sha={}&page={}&limit={}",
            owner, repo, params.branch, params.page, params.per_page
        );

        if let Some(since) = params.since {
            path.push_str(&format!("&since={}", since.to_rfc3339()));
        }

        if let Some(until) = params.until {
            path.push_str(&format!("&until={}", until.to_rfc3339()));
        }

        let commits: Vec<GiteaCommit> = self.get(&path).await?;
        Ok(commits.into_iter().map(Into::into).collect())
    }

    async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        branch: &str,
    ) -> ApiResult<FileContent> {
        let api_path = format!("/repos/{}/{}/contents/{}?ref={}", owner, repo, path, branch);
        let file: GiteaFileContent = self.get(&api_path).await?;
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
    async fn test_gitea_client_new() {
        let client = GiteaClient::new("token", "http://localhost").unwrap();
        assert_eq!(client.token, "token");
        assert_eq!(client.base_url, "http://localhost");
    }

    #[tokio::test]
    async fn test_gitea_client_as_collector() {
        let client = GiteaClient::new("token", "http://localhost").unwrap();
        let _collector = client.as_collector();
    }

    #[tokio::test]
    async fn test_get_repository() {
        let server = MockServer::start();
        let client = GiteaClient::new("token", server.base_url()).unwrap();

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
            "clone_url": "http://localhost/owner/test-repo.git",
            "owner": {
                "id": 1,
                "login": "owner",
                "full_name": "owner",
                "email": "owner@example.com",
                "avatar_url": "url",
                "language": "rust",
                "username": "owner"
            }
        });

        let _mock = server.mock(|when, then| {
            when.method(GET)
                .path("/repos/owner/test-repo")
                .header("Authorization", "token token");
            then.status(200).json_body(repo_response);
        });

        let result = client.get_repository("owner", "test-repo").await;
        // mock.assert();
        assert!(result.is_ok(), "Result error: {:?}", result.err());
        let repo = result.unwrap();
        assert_eq!(repo.name, "test-repo");
    }

    #[tokio::test]
    async fn test_get_branches() {
        let server = MockServer::start();
        let client = GiteaClient::new("token", server.base_url()).unwrap();

        let branch_response = json!([
            {
                "name": "main",
                "commit": {
                    "id": "sha",
                    "url": "url"
                },
                "protected": true
            }
        ]);

