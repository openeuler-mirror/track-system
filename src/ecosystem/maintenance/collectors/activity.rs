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
