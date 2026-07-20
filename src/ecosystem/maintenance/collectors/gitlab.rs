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
            .ok_or_else(|| anyhow!("package {} missing l0_repo_url", package.name))?;
        let repo_ref = parse_gitlab_repo(repo_url)
            .ok_or_else(|| anyhow!("failed to parse GitLab repo from {}", repo_url))?;

        let token = std::env::var("GITLAB_PRIVATE_TOKEN")
            .or_else(|_| std::env::var("GITLAB_TOKEN"))
            .or_else(|_| std::env::var("GITLAB_ACCESS_TOKEN"))
            .unwrap_or_default();
        let client = build_client()?;
        let activity_client = GitLabClient::with_base_url(repo_ref.api_base.clone(), token)?;

        info!(
            owner = repo_ref.owner,
            repo = repo_ref.repo,
            package = package.name,
            "开始采集 GitLab 平台维护元数据"
        );
        let project = fetch_project(&client, &repo_ref).await?;

        debug!(
            owner = repo_ref.owner,
            repo = repo_ref.repo,
            stars = project.star_count,
            forks = project.forks_count,
            "GitLab 平台维护元数据采集完成"
        );

        let mut evidence = vec![json!({
            "source_type": "gitlab_repository_metadata",
            "source_name": "gitlab_repository_metadata",
            "source_url": project.web_url,
            "http_status": 200,
            "assessment_category": "maintenance",
            "assessment_subcategory": "repository_metadata",
            "data": {
                "collector": "gitlab_live_api",
                "platform": "gitlab",
                "owner": repo_ref.owner,
                "repo": repo_ref.repo,
                "repo_html_url": project.web_url,
                "default_branch": project.default_branch,
                "stars": project.star_count,
                "forks": project.forks_count,
                "social_metrics_supported": true,
            }
        })];

        if let Some(branch) = project
            .default_branch
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            match collect_commit_activity(
                &activity_client,
                &repo_ref.owner,
                &repo_ref.repo,
                branch,
            )
            .await
            {
                Ok(activity) => evidence.push(json!({
                    "source_type": "gitlab_repository_activity_live",
                    "source_name": "gitlab_repository_activity",
                    "source_url": project.web_url,
                    "http_status": 200,
                    "assessment_category": "maintenance",
                    "assessment_subcategory": "repository_activity",
                    "data": {
                        "collector": "gitlab_live_api",
                        "platform": "gitlab",
                        "owner": repo_ref.owner,
                        "repo": repo_ref.repo,
