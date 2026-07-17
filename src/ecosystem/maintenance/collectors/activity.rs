use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use std::collections::BTreeSet;
use tracing::warn;

use crate::collectors::{Commit, CommitsParams, GitClient};

const DEFAULT_MAX_TOTAL_COUNT_PAGES: u32 = 500;
const DEFAULT_MAX_RECENT_ACTIVITY_PAGES: u32 = 200;
const COMMITS_PER_PAGE: u32 = 100;

#[derive(Debug, Clone, Default)]
pub struct RepositoryActivityMetrics {
    pub default_branch: Option<String>,
    pub last_commit_at: Option<String>,
    pub commit_total: i64,
    pub commit_total_is_lower_bound: bool,
    pub commits_last_12_months: i64,
    pub commits_last_12_months_is_lower_bound: bool,
    pub committers_last_12_months: i64,
}

pub async fn collect_commit_activity<C>(
    client: &C,
    owner: &str,
    repo: &str,
    branch: &str,
) -> Result<RepositoryActivityMetrics>
where
    C: GitClient + ?Sized,
{
    collect_commit_activity_with_limits(
        client,
        owner,
        repo,
        branch,
        DEFAULT_MAX_TOTAL_COUNT_PAGES,
        DEFAULT_MAX_RECENT_ACTIVITY_PAGES,
    )
    .await
}

async fn collect_commit_activity_with_limits<C>(
    client: &C,
    owner: &str,
    repo: &str,
    branch: &str,
    max_total_pages: u32,
    max_recent_pages: u32,
) -> Result<RepositoryActivityMetrics>
where
    C: GitClient + ?Sized,
{
    let since = Utc::now() - Duration::days(365);
    let latest_commit = client
        .get_commits(owner, repo, CommitsParams::new(branch).page(1).per_page(1))
        .await
        .with_context(|| format!("fetch latest commits failed for {owner}/{repo}"))?;
    let last_commit_at = latest_commit
        .first()
        .map(commit_timestamp)
        .map(|value| value.to_rfc3339());

    let (commit_total, commit_total_is_lower_bound) =
        count_commits(client, owner, repo, branch, None, max_total_pages).await?;
    let (recent_commits, commits_last_12_months_is_lower_bound, recent_committers) =
        collect_recent_activity(client, owner, repo, branch, since, max_recent_pages).await?;

    Ok(RepositoryActivityMetrics {
        default_branch: Some(branch.to_string()),
        last_commit_at,
        commit_total,
        commit_total_is_lower_bound,
        commits_last_12_months: recent_commits,
        commits_last_12_months_is_lower_bound,
        committers_last_12_months: recent_committers,
    })
}

async fn count_commits<C>(
    client: &C,
    owner: &str,
    repo: &str,
    branch: &str,
    since: Option<DateTime<Utc>>,
    max_pages: u32,
) -> Result<(i64, bool)>
where
    C: GitClient + ?Sized,
{
    let mut total = 0_i64;

    for page in 1..=max_pages {
