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
        let mut params = CommitsParams::new(branch)
            .page(page)
            .per_page(COMMITS_PER_PAGE);
        if let Some(since) = since {
            params = params.since(since);
        }

        let commits = client
            .get_commits(owner, repo, params)
            .await
            .with_context(|| format!("count commits failed for {owner}/{repo} page {page}"))?;
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
        repo, branch, max_pages, "平台 API commit 计数达到页数上限，返回下界"
    );
    Ok((total, true))
}

async fn collect_recent_activity<C>(
    client: &C,
    owner: &str,
    repo: &str,
    branch: &str,
    since: DateTime<Utc>,
    max_pages: u32,
) -> Result<(i64, bool, i64)>
where
    C: GitClient + ?Sized,
{
    let mut total = 0_i64;
    let mut identities = BTreeSet::new();

    for page in 1..=max_pages {
        let params = CommitsParams::new(branch)
            .since(since)
            .page(page)
            .per_page(COMMITS_PER_PAGE);
        let commits = client
            .get_commits(owner, repo, params)
            .await
            .with_context(|| {
                format!("collect recent activity failed for {owner}/{repo} page {page}")
            })?;

        if commits.is_empty() {
            return Ok((total, false, identities.len() as i64));
        }

        total += commits.len() as i64;
        for commit in &commits {
            identities.insert(normalized_commit_identity(commit));
        }

        if commits.len() < COMMITS_PER_PAGE as usize {
            return Ok((total, false, identities.len() as i64));
        }
    }

    warn!(
        owner,
        repo, branch, max_pages, "平台 API 近 12 个月 commit 活跃度统计达到页数上限，返回下界"
    );
    Ok((total, true, identities.len() as i64))
}

pub fn normalized_commit_identity(commit: &Commit) -> String {
    if !commit.committer_email.trim().is_empty() {
        return commit.committer_email.to_ascii_lowercase();
    }
    if !commit.author_email.trim().is_empty() {
        return commit.author_email.to_ascii_lowercase();
    }
    if !commit.committer_name.trim().is_empty() {
        return format!("name:{}", commit.committer_name);
    }
    if !commit.author_name.trim().is_empty() {
        return format!("name:{}", commit.author_name);
    }
    format!("sha:{}", commit.sha)
}

