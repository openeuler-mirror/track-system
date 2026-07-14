use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration, Utc};
use reqwest::Url;
use reqwest::{header, Client, Response, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info};

use crate::entities::packages;

const GITHUB_API_BASE: &str = "https://api.github.com";
const DEFAULT_TIMEOUT_SECS: u64 = 40;
const MAX_COMMITTER_PAGES: u32 = 200;

#[derive(Debug, Default, Clone)]
pub struct GitHubMaintenanceCollector;

#[derive(Debug, Deserialize)]
struct GitHubRepositorySnapshot {
    html_url: String,
    default_branch: String,
    stargazers_count: i64,
    forks_count: i64,
}

#[derive(Debug, Deserialize)]
struct GitHubCommitListItem {
    sha: String,
    commit: GitHubCommitInfo,
}

#[derive(Debug, Deserialize)]
struct GitHubCommitInfo {
    author: Option<GitHubCommitIdentity>,
    committer: Option<GitHubCommitIdentity>,
}

#[derive(Debug, Deserialize)]
struct GitHubCommitIdentity {
    name: Option<String>,
    email: Option<String>,
    date: Option<DateTime<Utc>>,
}

struct GitHubApi {
    client: Client,
    token: Option<String>,
    base_url: String,
}

impl GitHubMaintenanceCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn matches_package(package: &packages::Model) -> bool {
        package
            .l0_repo_url
            .as_deref()
            .and_then(parse_github_repo)
            .is_some()
    }

    pub async fn collect(&self, package: &packages::Model) -> Result<Vec<Value>> {
        if !Self::matches_package(package) {
            return Ok(Vec::new());
        }

        let repo_url = package
            .l0_repo_url
            .as_deref()
            .ok_or_else(|| anyhow!("package {} missing l0_repo_url", package.name))?;
        let (owner, repo) = parse_github_repo(repo_url)
            .ok_or_else(|| anyhow!("failed to parse GitHub repo from {}", repo_url))?;
        let api = GitHubApi::new(Some(GITHUB_API_BASE.to_string()))?;

        info!(
            owner,
            repo,
            package = package.name,
            "开始采集 GitHub 组件维护指标"
        );

        self.collect_with_api(package, repo_url, owner, repo, api)
            .await
    }

    async fn collect_with_api(
        &self,
        package: &packages::Model,
        repo_url: &str,
        owner: String,
        repo: String,
        api: GitHubApi,
    ) -> Result<Vec<Value>> {
        let repo_info = api.fetch_repository(&owner, &repo).await?;
        let branch = repo_info.default_branch.clone();
        let since = Utc::now() - Duration::days(365);

        let commit_total = api.count_commits(&owner, &repo, &branch, None).await?;
        let commits_last_12_months = api
            .count_commits(&owner, &repo, &branch, Some(since))
            .await?;
        let committers_last_12_months = api
            .count_unique_committers_since(&owner, &repo, &branch, since)
            .await?;
        let last_commit = api.fetch_latest_commit(&owner, &repo, &branch).await?;
        let last_commit_at = last_commit
            .as_ref()
            .and_then(|commit| commit.commit.committer.as_ref())
            .and_then(|identity| identity.date)
            .or_else(|| {
                last_commit
                    .as_ref()
                    .and_then(|commit| commit.commit.author.as_ref())
                    .and_then(|identity| identity.date)
            })
            .map(|dt| dt.to_rfc3339());

        debug!(
