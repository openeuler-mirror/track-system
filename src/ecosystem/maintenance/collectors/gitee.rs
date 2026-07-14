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

impl GiteeMaintenanceCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn matches_package(package: &packages::Model) -> bool {
        package
            .l0_repo_url
            .as_deref()
            .and_then(parse_gitee_repo)
            .is_some()
    }

    pub async fn collect(&self, package: &packages::Model) -> Result<Vec<Value>> {
        let repo_url = package
            .l0_repo_url
            .as_deref()
            .ok_or_else(|| anyhow!("package {} missing l0_repo_url", package.name))?;
        let (owner, repo) = parse_gitee_repo(repo_url)
            .ok_or_else(|| anyhow!("failed to parse Gitee repo from {}", repo_url))?;

        info!(
            owner,
            repo,
            package = package.name,
            "开始采集 Gitee 平台维护元数据"
        );

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .user_agent("track-system/maintenance-gitee")
            .build()
            .context("build gitee maintenance client failed")?;

        let repo_info = fetch_repository(&client, &owner, &repo).await?;
        let activity_client =
            GiteeClient::new(std::env::var("GITEE_ACCESS_TOKEN").unwrap_or_default())?;

        debug!(
            owner = owner,
            repo = repo,
            stars = repo_info.stargazers_count,
            forks = repo_info.forks_count,
            "Gitee 平台维护元数据采集完成"
        );

        let mut evidence = vec![json!({
            "source_type": "gitee_repository_metadata",
            "source_name": "gitee_repository_metadata",
            "source_url": repo_info.html_url,
            "http_status": 200,
            "assessment_category": "maintenance",
            "assessment_subcategory": "repository_metadata",
            "data": {
                "collector": "gitee_live_api",
                "platform": "gitee",
                "owner": owner,
                "repo": repo,
                "repo_html_url": repo_info.html_url,
                "default_branch": repo_info.default_branch,
                "stars": repo_info.stargazers_count,
                "forks": repo_info.forks_count,
                "social_metrics_supported": true,
            }
        })];

        if let Some(branch) = repo_info
            .default_branch
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            match collect_commit_activity(&activity_client, &owner, &repo, branch).await {
                Ok(activity) => evidence.push(json!({
                    "source_type": "gitee_repository_activity_live",
                    "source_name": "gitee_repository_activity",
                    "source_url": repo_info.html_url,
                    "http_status": 200,
                    "assessment_category": "maintenance",
                    "assessment_subcategory": "repository_activity",
                    "data": {
                        "collector": "gitee_live_api",
                        "platform": "gitee",
                        "owner": owner,
                        "repo": repo,
                        "repo_html_url": repo_info.html_url,
                        "default_branch": activity.default_branch,
                        "commit_total": activity.commit_total,
                        "commit_total_is_lower_bound": activity.commit_total_is_lower_bound,
                        "commits_last_12_months": activity.commits_last_12_months,
                        "commits_last_12_months_is_lower_bound": activity.commits_last_12_months_is_lower_bound,
                        "committers_last_12_months": activity.committers_last_12_months,
                        "last_commit_at": activity.last_commit_at,
                        "stars": repo_info.stargazers_count,
                        "forks": repo_info.forks_count,
                    }
                })),
                Err(error) => warn!(
                    owner = owner,
                    repo = repo,
                    package = package.name,
                    error = %error,
                    "Gitee 活跃度指标采集失败，仅返回平台元数据"
                ),
            }
        }

        Ok(evidence)
    }
}

async fn fetch_repository(
    client: &Client,
    owner: &str,
    repo: &str,
