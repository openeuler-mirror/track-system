use anyhow::{anyhow, Context, Result};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info, warn};

use crate::collectors::GitLabClient;
use crate::entities::packages;

use super::activity::collect_commit_activity;

#[derive(Debug, Default, Clone)]
pub struct GitLabMaintenanceCollector;

const DEFAULT_TIMEOUT_SECS: u64 = 40;

#[derive(Debug, Deserialize)]
struct GitLabProjectSnapshot {
    web_url: String,
    default_branch: Option<String>,
    star_count: Option<i64>,
    forks_count: Option<i64>,
}

