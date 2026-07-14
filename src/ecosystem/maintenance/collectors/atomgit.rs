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
