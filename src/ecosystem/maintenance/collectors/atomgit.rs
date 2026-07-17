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
