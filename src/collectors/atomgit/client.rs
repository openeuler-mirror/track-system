use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;
use tracing::debug;

use crate::collectors::{
    error::{ApiError, ApiResult},
    traits::{
        Branch, Collector, Commit, CommitsParams, FileContent, GitClient, Issue, IssueClient,
        IssueParams, Repository,
    },
};

use super::models::{
    AtomGitBranch, AtomGitCommit, AtomGitError, AtomGitFileContent, AtomGitIssue, AtomGitRepository,
};

const ATOMGIT_API_BASE: &str = "https://api.atomgit.com/api/v5";
const DEFAULT_BRANCH: &str = "master";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u32 = 3;

pub struct AtomGitClient {
    client: Client,
    token: String,
    base_url: String,
    default_branch: String,
}

impl AtomGitClient {
    pub fn new(token: impl Into<String>, default_branch: impl Into<String>) -> ApiResult<Self> {
        let client = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .user_agent("track-system/0.1.0")
            .no_proxy()
            .build()?;

        Ok(Self {
            client,
            token: token.into(),
            base_url: ATOMGIT_API_BASE.to_string(),
            default_branch: default_branch.into(),
        })
    }

    pub fn with_config(
        token: impl Into<String>,
        default_branch: impl Into<String>,
        timeout: Duration,
    ) -> ApiResult<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .user_agent("track-system/0.1.0")
            .no_proxy()
            .build()?;

        Ok(Self {
            client,
            token: token.into(),
            base_url: ATOMGIT_API_BASE.to_string(),
            default_branch: default_branch.into(),
        })
    }

    pub fn for_testing(
        token: impl Into<String>,
        default_branch: impl Into<String>,
        base_url: impl Into<String>,
    ) -> ApiResult<Self> {
        let client = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .user_agent("track-system/0.1.0")
            .no_proxy()
            .build()?;

        Ok(Self {
            client,
            token: token.into(),
            base_url: base_url.into(),
            default_branch: default_branch.into(),
        })
    }

    pub fn as_collector(self) -> impl Collector {
        use crate::collectors::{adapters::GitClientCollectorAdapter, traits::Platform};
        GitClientCollectorAdapter::new(self, Platform::AtomGit)
    }

    pub async fn get_commit_detail(&self, owner: &str, repo: &str, sha: &str) -> ApiResult<Commit> {
        let url = format!(
            "{}/repos/{}/{}/commits/{}?access_token={}",
            self.base_url, owner, repo, sha, self.token
        );
        let atomgit_commit: AtomGitCommit = self.get(&url).await?;
        Ok(atomgit_commit.into())
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> ApiResult<T> {
        let mut retries = 0;

        loop {
            let response = self.client.get(url).send().await?;

            let status = response.status();

            debug!("AtomGit API GET {}: {}", url, status);
            if status.is_success() {
                return response.json::<T>().await.map_err(ApiError::from);
            }

            let error_body = response.text().await.unwrap_or_default();

            let error_message = serde_json::from_str::<AtomGitError>(&error_body)
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
impl GitClient for AtomGitClient {
    async fn get_repository(&self, owner: &str, repo: &str) -> ApiResult<Repository> {
        let branch = if self.default_branch.is_empty() {
            DEFAULT_BRANCH
        } else {
            self.default_branch.as_str()
        };
        let url = format!(
            "{}/repos/{}/{}/branches/{}?access_token={}",
            self.base_url, owner, repo, branch, self.token
        );
        let atomgit_repo: AtomGitRepository = self.get(&url).await?;
        Ok(atomgit_repo.into())
    }

    async fn get_branches(&self, owner: &str, repo: &str) -> ApiResult<Vec<Branch>> {
        let url = format!(
            "{}/repos/{}/{}/branches?access_token={}",
            self.base_url, owner, repo, self.token
        );
        let atomgit_branches: Vec<AtomGitBranch> = self.get(&url).await?;
        Ok(atomgit_branches.into_iter().map(Into::into).collect())
    }

    async fn get_commits(
        &self,
        owner: &str,
        repo: &str,
        params: CommitsParams,
    ) -> ApiResult<Vec<Commit>> {
        let mut url = format!(
            "{}/repos/{}/{}/commits?access_token={}&sha={}&page={}&per_page={}",
            self.base_url, owner, repo, self.token, params.branch, params.page, params.per_page
        );

        if let Some(since) = params.since {
            url.push_str(&format!("&since={}", since.to_rfc3339()));
        }

        if let Some(until) = params.until {
            url.push_str(&format!("&until={}", until.to_rfc3339()));
        }

        let atomgit_commits: Vec<AtomGitCommit> = self.get(&url).await?;

        let mut commit_vec = Vec::new();
        for commit in &atomgit_commits {
            let commit_detail = self.get_commit_detail(owner, repo, &commit.sha).await?;
            debug!("AtomGit API GET commit detail: {:?}", commit_detail);
            commit_vec.push(commit_detail);
        }

        Ok(commit_vec.into_iter().map(Into::into).collect())
    }

    async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        branch: &str,
    ) -> ApiResult<FileContent> {
        let url = format!(
            "{}/repos/{}/{}/contents/{}?access_token={}&ref={}",
            self.base_url, owner, repo, path, self.token, branch
        );
        let atomgit_file: AtomGitFileContent = self.get(&url).await?;
        Ok(atomgit_file.into())
    }
}

#[async_trait]
impl IssueClient for AtomGitClient {
    async fn get_issues(
        &self,
        owner: &str,
        repo: &str,
        params: IssueParams,
    ) -> ApiResult<Vec<Issue>> {
        let mut url = format!(
            "{}/repos/{}/{}/issues?access_token={}&page={}&per_page={}&state={}",
            self.base_url,
            owner,
            repo,
            self.token,
            params.page,
            params.per_page,
            params.state.as_query_value()
        );

        if let Some(since) = params.since {
            url.push_str(&format!("&since={}", since.to_rfc3339()));
        }

        let issues: Vec<AtomGitIssue> = self.get(&url).await?;
        Ok(issues.into_iter().map(Into::into).collect())
    }
}
