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
                        "repo_html_url": project.web_url,
                        "default_branch": activity.default_branch,
                        "commit_total": activity.commit_total,
                        "commit_total_is_lower_bound": activity.commit_total_is_lower_bound,
                        "commits_last_12_months": activity.commits_last_12_months,
                        "commits_last_12_months_is_lower_bound": activity.commits_last_12_months_is_lower_bound,
                        "committers_last_12_months": activity.committers_last_12_months,
                        "last_commit_at": activity.last_commit_at,
                        "stars": project.star_count,
                        "forks": project.forks_count,
                    }
                })),
                Err(error) => warn!(
                    owner = repo_ref.owner,
                    repo = repo_ref.repo,
                    package = package.name,
                    error = %error,
                    "GitLab 活跃度指标采集失败，仅返回平台元数据"
                ),
            }
        }

        Ok(evidence)
    }
}

fn build_client() -> Result<Client> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
        .user_agent("track-system/maintenance-gitlab")
        .build()
        .context("build gitlab maintenance client failed")
}

async fn fetch_project(client: &Client, repo_ref: &GitLabRepoRef) -> Result<GitLabProjectSnapshot> {
    let encoded = urlencoding::encode(&repo_ref.project_path);
    let url = format!("{}/projects/{}", repo_ref.api_base, encoded);
    let mut request = client.get(&url);

    if let Ok(token) = std::env::var("GITLAB_PRIVATE_TOKEN")
        .or_else(|_| std::env::var("GITLAB_TOKEN"))
        .or_else(|_| std::env::var("GITLAB_ACCESS_TOKEN"))
    {
        if !token.trim().is_empty() {
            request = request.header("PRIVATE-TOKEN", token);
        }
    }

    let response = request
        .send()
        .await
        .context("send gitlab project request failed")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("GitLab API HTTP {}: {}", status.as_u16(), body));
    }

    response
        .json::<GitLabProjectSnapshot>()
        .await
        .context("parse gitlab project response failed")
}

fn parse_gitlab_repo(url: &str) -> Option<GitLabRepoRef> {
    let normalized = normalize_repo_url(url)?;
    let host = normalized.host_str()?.to_string();
    if host != "gitlab.com" && !host.starts_with("gitlab.") {
        return None;
    }

    let segments = normalized
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.trim_end_matches(".git").to_string())
        .collect::<Vec<_>>();
    if segments.len() < 2 {
        return None;
    }

    let repo = segments.last()?.to_string();
    let owner = segments[..segments.len() - 1].join("/");

    Some(GitLabRepoRef {
        api_base: format!("{}://{}/api/v4", normalized.scheme(), host),
        project_path: segments.join("/"),
        owner,
        repo,
    })
}

fn normalize_repo_url(url: &str) -> Option<Url> {
    if let Ok(parsed) = Url::parse(url) {
        return Some(parsed);
