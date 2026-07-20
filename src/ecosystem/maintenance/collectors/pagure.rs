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
            "assessment_category": "maintenance",
            "assessment_subcategory": "repository_metadata",
            "data": {
                "collector": "pagure_live_api",
                "platform": repo_ref.platform,
                "owner": owner,
                "repo": repo,
                "repo_html_url": project.full_url,
                "namespace": project.namespace,
                "social_metrics_supported": false,
                "stars": null,
                "forks": null,
                "default_branch": null,
                "last_project_modified_at": project.date_modified,
            }
            }),
            json!({
                "source_type": "pagure_repository_activity_live",
                "source_name": "pagure_repository_activity",
                "source_url": project.full_url,
                "http_status": 200,
                "assessment_category": "maintenance",
                "assessment_subcategory": "repository_activity",
                "data": {
                    "collector": "pagure_live_api",
                    "platform": repo_ref.platform,
                    "owner": owner,
                    "repo": repo,
                    "repo_html_url": project.full_url,
                    "default_branch": null,
                    "commit_total": null,
                    "commits_last_12_months": null,
                    "committers_last_12_months": null,
                    "last_commit_at": project.date_modified,
                    "stars": null,
                    "forks": null,
                }
            }),
        ])
    }
}

async fn fetch_project(client: &Client, api_url: &str) -> Result<PagureProjectSnapshot> {
    let response = client
        .get(api_url)
        .send()
        .await
        .context("send pagure project request failed")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Pagure API HTTP {}: {}", status.as_u16(), body));
    }

    response
        .json::<PagureProjectSnapshot>()
        .await
        .context("parse pagure project response failed")
}

fn parse_pagure_repo(url: &str) -> Option<PagureRepoRef> {
    let normalized = normalize_url(url)?;
    let host = normalized.host_str()?;
    let segments = normalized
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.trim_end_matches(".git").to_string())
        .collect::<Vec<_>>();

    match host {
