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

struct GitLabRepoRef {
    api_base: String,
    project_path: String,
    owner: String,
    repo: String,
}

impl GitLabMaintenanceCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn matches_package(package: &packages::Model) -> bool {
        package
            .l0_repo_url
            .as_deref()
            .and_then(parse_gitlab_repo)
            .is_some()
    }

    pub async fn collect(&self, package: &packages::Model) -> Result<Vec<Value>> {
        let repo_url = package
            .l0_repo_url
            .as_deref()
