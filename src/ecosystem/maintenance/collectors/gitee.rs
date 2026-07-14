use anyhow::{anyhow, Context, Result};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info, warn};

use crate::collectors::GiteeClient;
use crate::entities::packages;

use super::activity::collect_commit_activity;

const GITEE_API_BASE: &str = "https://gitee.com/api/v5";
const DEFAULT_TIMEOUT_SECS: u64 = 40;

#[derive(Debug, Default, Clone)]
pub struct GiteeMaintenanceCollector;

#[derive(Debug, Deserialize)]
struct GiteeRepositorySnapshot {
    html_url: String,
    default_branch: Option<String>,
    stargazers_count: Option<i64>,
    forks_count: Option<i64>,
}
