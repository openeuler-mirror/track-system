use anyhow::{anyhow, Context, Result};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info};

use crate::entities::packages;

const DEFAULT_TIMEOUT_SECS: u64 = 40;

#[derive(Debug, Default, Clone)]
pub struct PagureMaintenanceCollector;

#[derive(Debug, Deserialize)]
struct PagureProjectSnapshot {
    full_url: String,
    fullname: String,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    date_modified: Option<String>,
    #[serde(default)]
    user: Option<PagureUser>,
}

#[derive(Debug, Deserialize)]
struct PagureUser {
    name: String,
}

struct PagureRepoRef {
    api_url: String,
    owner: String,
    repo: String,
    platform: &'static str,
}

impl PagureMaintenanceCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn matches_package(package: &packages::Model) -> bool {
        package
            .l0_repo_url
            .as_deref()
            .and_then(parse_pagure_repo)
            .is_some()
    }

    pub async fn collect(&self, package: &packages::Model) -> Result<Vec<Value>> {
        let repo_url = package
            .l0_repo_url
            .as_deref()
            .ok_or_else(|| anyhow!("package {} missing l0_repo_url", package.name))?;
        let repo_ref = parse_pagure_repo(repo_url)
            .ok_or_else(|| anyhow!("failed to parse Pagure/Fedora repo from {}", repo_url))?;

        info!(
            owner = repo_ref.owner,
            repo = repo_ref.repo,
            package = package.name,
            platform = repo_ref.platform,
            "开始采集 Pagure/Fedora 平台维护元数据"
        );

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .user_agent("track-system/maintenance-pagure")
            .build()
