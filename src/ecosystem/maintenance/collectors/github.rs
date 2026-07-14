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
