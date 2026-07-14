use anyhow::{anyhow, Context, Result};
use chrono::{Duration, Utc};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info, warn};

use crate::collectors::{atomgit::models::AtomGitCommit, Commit};
use crate::entities::packages;

use super::activity::{commit_timestamp, normalized_commit_identity, RepositoryActivityMetrics};

const ATOMGIT_API_BASE: &str = "https://api.atomgit.com/api/v5";
const DEFAULT_TIMEOUT_SECS: u64 = 40;
const COMMITS_PER_PAGE: u32 = 100;
const MAX_TOTAL_COUNT_PAGES: u32 = 500;
const MAX_RECENT_ACTIVITY_PAGES: u32 = 200;

#[derive(Debug, Default, Clone)]
pub struct AtomGitMaintenanceCollector;

#[derive(Debug, Deserialize)]
struct AtomGitRepositorySnapshot {
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    default_branch: Option<String>,
    #[serde(default)]
    stargazers_count: Option<i64>,
    #[serde(default)]
    forks_count: Option<i64>,
}

impl AtomGitMaintenanceCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn matches_package(package: &packages::Model) -> bool {
        package
            .l0_repo_url
            .as_deref()
            .and_then(parse_atomgit_repo)
            .is_some()
    }

    pub async fn collect(&self, package: &packages::Model) -> Result<Vec<Value>> {
        let repo_url = package
            .l0_repo_url
            .as_deref()
            .ok_or_else(|| anyhow!("package {} missing l0_repo_url", package.name))?;
        let (owner, repo) = parse_atomgit_repo(repo_url)
            .ok_or_else(|| anyhow!("failed to parse AtomGit repo from {}", repo_url))?;
        let token = std::env::var("ATOMGIT_TOKEN")
            .or_else(|_| std::env::var("GITCODE_TOKEN"))
            .or_else(|_| std::env::var("GITCODE_ACCESS_TOKEN"))
            .ok()
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| anyhow!("ATOMGIT_TOKEN is required for AtomGit maintenance metadata"))?;

        info!(
            owner,
            repo,
            package = package.name,
            "开始采集 AtomGit 平台维护元数据"
        );

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .user_agent("track-system/maintenance-atomgit")
            .build()
            .context("build atomgit maintenance client failed")?;

        let repo_info = fetch_repository(&client, &token, &owner, &repo).await?;
        let html_url = repo_info
            .html_url
            .unwrap_or_else(|| normalize_source_url(repo_url));

        debug!(
            owner = owner,
            repo = repo,
            stars = repo_info.stargazers_count,
            forks = repo_info.forks_count,
            "AtomGit 平台维护元数据采集完成"
        );

        let mut evidence = vec![json!({
            "source_type": "atomgit_repository_metadata",
            "source_name": "atomgit_repository_metadata",
            "source_url": html_url,
            "http_status": 200,
            "assessment_category": "maintenance",
            "assessment_subcategory": "repository_metadata",
            "data": {
                "collector": "atomgit_live_api",
                "platform": "atomgit",
                "owner": owner,
                "repo": repo,
                "repo_html_url": html_url,
                "default_branch": repo_info.default_branch,
                "stars": repo_info.stargazers_count,
                "forks": repo_info.forks_count,
                "social_metrics_supported": true,
            }
        })];

        if let Some(branch) = repo_info
            .default_branch
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            match collect_activity(&client, &token, &owner, &repo, branch).await {
                Ok(activity) => evidence.push(json!({
                    "source_type": "atomgit_repository_activity_live",
                    "source_name": "atomgit_repository_activity",
                    "source_url": html_url,
                    "http_status": 200,
                    "assessment_category": "maintenance",
                    "assessment_subcategory": "repository_activity",
                    "data": {
                        "collector": "atomgit_live_api",
                        "platform": "atomgit",
                        "owner": owner,
                        "repo": repo,
                        "repo_html_url": html_url,
                        "default_branch": activity.default_branch,
                        "commit_total": activity.commit_total,
                        "commit_total_is_lower_bound": activity.commit_total_is_lower_bound,
                        "commits_last_12_months": activity.commits_last_12_months,
                        "commits_last_12_months_is_lower_bound": activity.commits_last_12_months_is_lower_bound,
                        "committers_last_12_months": activity.committers_last_12_months,
                        "last_commit_at": activity.last_commit_at,
                        "stars": repo_info.stargazers_count,
                        "forks": repo_info.forks_count,
                    }
                })),
                Err(error) => warn!(
                    owner = owner,
                    repo = repo,
                    package = package.name,
                    error = %error,
                    "AtomGit 活跃度指标采集失败，仅返回平台元数据"
                ),
            }
        }

        Ok(evidence)
    }
}

async fn fetch_repository(
    client: &Client,
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<AtomGitRepositorySnapshot> {
    let url = format!("{}/repos/{}/{}", ATOMGIT_API_BASE, owner, repo);
    let response = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .context("send atomgit repository request failed")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("AtomGit API HTTP {}: {}", status.as_u16(), body));
    }

    response
        .json::<AtomGitRepositorySnapshot>()
        .await
        .context("parse atomgit repository response failed")
}

async fn collect_activity(
    client: &Client,
    token: &str,
    owner: &str,
    repo: &str,
    branch: &str,
) -> Result<RepositoryActivityMetrics> {
    let latest = fetch_commit_page(client, token, owner, repo, branch, None, 1, 1).await?;
    let last_commit_at = latest
        .first()
        .map(commit_timestamp)
        .map(|value| value.to_rfc3339());
    let (commit_total, commit_total_is_lower_bound) = count_commits(
        client,
        token,
        owner,
        repo,
        branch,
        None,
        MAX_TOTAL_COUNT_PAGES,
    )
    .await?;
    let since = Utc::now() - Duration::days(365);
    let (commits_last_12_months, commits_last_12_months_is_lower_bound, committers_last_12_months) =
        collect_recent_activity(
            client,
            token,
            owner,
            repo,
            branch,
            since,
            MAX_RECENT_ACTIVITY_PAGES,
        )
        .await?;

    Ok(RepositoryActivityMetrics {
        default_branch: Some(branch.to_string()),
        last_commit_at,
        commit_total,
        commit_total_is_lower_bound,
        commits_last_12_months,
        commits_last_12_months_is_lower_bound,
        committers_last_12_months,
    })
}

async fn count_commits(
    client: &Client,
    token: &str,
    owner: &str,
    repo: &str,
    branch: &str,
    since: Option<chrono::DateTime<Utc>>,
    max_pages: u32,
) -> Result<(i64, bool)> {
    let mut total = 0_i64;

    for page in 1..=max_pages {
        let commits = fetch_commit_page(
            client,
            token,
            owner,
            repo,
            branch,
            since,
            page,
            COMMITS_PER_PAGE,
        )
        .await?;
        if commits.is_empty() {
            return Ok((total, false));
        }

        total += commits.len() as i64;
        if commits.len() < COMMITS_PER_PAGE as usize {
            return Ok((total, false));
        }
    }

    warn!(
        owner,
        repo, branch, max_pages, "AtomGit commit 计数达到页数上限，返回下界"
    );
    Ok((total, true))
}

async fn collect_recent_activity(
    client: &Client,
    token: &str,
    owner: &str,
    repo: &str,
    branch: &str,
    since: chrono::DateTime<Utc>,
    max_pages: u32,
) -> Result<(i64, bool, i64)> {
    let mut total = 0_i64;
    let mut identities = std::collections::BTreeSet::new();

    for page in 1..=max_pages {
        let commits = fetch_commit_page(
            client,
            token,
            owner,
            repo,
            branch,
            Some(since),
            page,
            COMMITS_PER_PAGE,
        )
        .await?;
