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
            .context("build pagure maintenance client failed")?;

        let project = fetch_project(&client, &repo_ref.api_url).await?;
        let owner = project
            .user
            .as_ref()
            .map(|user| user.name.clone())
            .unwrap_or_else(|| repo_ref.owner.clone());
        let repo = project.name.unwrap_or_else(|| repo_ref.repo.clone());

        debug!(
            owner = owner,
            repo = repo,
            fullname = project.fullname,
            platform = repo_ref.platform,
            "Pagure/Fedora 平台维护元数据采集完成"
        );

        Ok(vec![
            json!({
            "source_type": "pagure_repository_metadata",
            "source_name": "pagure_repository_metadata",
            "source_url": project.full_url,
            "http_status": 200,
