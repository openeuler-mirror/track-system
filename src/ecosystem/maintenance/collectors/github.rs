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
}

impl GitHubMaintenanceCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn matches_package(package: &packages::Model) -> bool {
        package
            .l0_repo_url
            .as_deref()
            .and_then(parse_github_repo)
            .is_some()
    }

    pub async fn collect(&self, package: &packages::Model) -> Result<Vec<Value>> {
        if !Self::matches_package(package) {
            return Ok(Vec::new());
        }

        let repo_url = package
            .l0_repo_url
            .as_deref()
            .ok_or_else(|| anyhow!("package {} missing l0_repo_url", package.name))?;
        let (owner, repo) = parse_github_repo(repo_url)
            .ok_or_else(|| anyhow!("failed to parse GitHub repo from {}", repo_url))?;
        let api = GitHubApi::new(Some(GITHUB_API_BASE.to_string()))?;

        info!(
            owner,
            repo,
            package = package.name,
            "开始采集 GitHub 组件维护指标"
        );

        self.collect_with_api(package, repo_url, owner, repo, api)
            .await
    }

    async fn collect_with_api(
        &self,
        package: &packages::Model,
        repo_url: &str,
        owner: String,
        repo: String,
        api: GitHubApi,
    ) -> Result<Vec<Value>> {
        let repo_info = api.fetch_repository(&owner, &repo).await?;
        let branch = repo_info.default_branch.clone();
        let since = Utc::now() - Duration::days(365);

        let commit_total = api.count_commits(&owner, &repo, &branch, None).await?;
        let commits_last_12_months = api
            .count_commits(&owner, &repo, &branch, Some(since))
            .await?;
        let committers_last_12_months = api
            .count_unique_committers_since(&owner, &repo, &branch, since)
            .await?;
        let last_commit = api.fetch_latest_commit(&owner, &repo, &branch).await?;
        let last_commit_at = last_commit
            .as_ref()
            .and_then(|commit| commit.commit.committer.as_ref())
            .and_then(|identity| identity.date)
            .or_else(|| {
                last_commit
                    .as_ref()
                    .and_then(|commit| commit.commit.author.as_ref())
                    .and_then(|identity| identity.date)
            })
            .map(|dt| dt.to_rfc3339());

        debug!(
            owner = owner,
            repo = repo,
            package = package.name,
            branch,
            commit_total,
            commits_last_12_months,
            committers_last_12_months,
            stars = repo_info.stargazers_count,
            forks = repo_info.forks_count,
            "GitHub 组件维护指标采集完成"
        );

        Ok(vec![json!({
            "source_type": "github_repository_activity_live",
            "source_name": "github_repository_activity",
            "source_url": repo_info.html_url,
            "http_status": 200,
            "assessment_category": "maintenance",
            "assessment_subcategory": "repository_activity",
            "data": {
                "collector": "github_live_api",
                "platform": "github",
                "owner": owner,
                "repo": repo,
                "repo_html_url": repo_info.html_url,
                "input_repo_url": repo_url,
                "default_branch": branch,
                "commit_total": commit_total,
                "commits_last_12_months": commits_last_12_months,
                "committers_last_12_months": committers_last_12_months,
                "last_commit_at": last_commit_at,
                "stars": repo_info.stargazers_count,
                "forks": repo_info.forks_count,
            }
        })])
    }
}

impl GitHubApi {
    fn new(base_url: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .user_agent("track-system/maintenance-github")
            .build()
            .context("build github maintenance client failed")?;

        Ok(Self {
            client,
            token: std::env::var("GITHUB_TOKEN")
                .or_else(|_| std::env::var("GITHUB_ACCESS_TOKEN"))
                .ok(),
            base_url: base_url.unwrap_or_else(|| GITHUB_API_BASE.to_string()),
        })
    }

    async fn fetch_repository(&self, owner: &str, repo: &str) -> Result<GitHubRepositorySnapshot> {
        let url = format!("{}/repos/{}/{}", self.base_url, owner, repo);
        self.get_json(&url).await
    }

    async fn fetch_latest_commit(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<Option<GitHubCommitListItem>> {
        let url = format!(
            "{}/repos/{}/{}/commits?sha={}&per_page=1&page=1",
            self.base_url, owner, repo, branch
        );
        let commits: Vec<GitHubCommitListItem> = self.get_json(&url).await?;
        Ok(commits.into_iter().next())
    }

    async fn count_commits(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
        since: Option<DateTime<Utc>>,
    ) -> Result<i64> {
        let mut url = format!(
            "{}/repos/{}/{}/commits?sha={}&per_page=1&page=1",
            self.base_url, owner, repo, branch
        );
        if let Some(since) = since {
            url.push_str(&format!("&since={}", since.to_rfc3339()));
        }

        let response = self.send(&url).await?;
        match response.status() {
            StatusCode::OK => {
                let headers = response.headers().clone();
                let commits: Vec<GitHubCommitListItem> = response
                    .json()
                    .await
                    .context("parse github commits response failed")?;
                if commits.is_empty() {
                    return Ok(0);
                }
                if let Some(link) = headers
                    .get(header::LINK)
                    .and_then(|value| value.to_str().ok())
                {
                    if let Some(last_page) = parse_last_page_from_link(link) {
                        return Ok(last_page as i64);
                    }
                }
                Ok(commits.len() as i64)
            }
            StatusCode::CONFLICT => Ok(0),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(anyhow!("GitHub API HTTP {}: {}", status.as_u16(), body))
            }
        }
    }

    async fn count_unique_committers_since(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
        since: DateTime<Utc>,
    ) -> Result<i64> {
        let mut unique_committers = std::collections::BTreeSet::new();
        let mut page = 1;

        loop {
            if page > MAX_COMMITTER_PAGES {
                break;
            }

            let url = format!(
                "{}/repos/{}/{}/commits?sha={}&since={}&per_page=100&page={}",
                self.base_url,
                owner,
                repo,
                branch,
                since.to_rfc3339(),
                page
            );
            let commits: Vec<GitHubCommitListItem> = self.get_json(&url).await?;
            if commits.is_empty() {
                break;
            }

            for commit in &commits {
                if let Some(identity) = normalized_committer_identity(commit) {
                    unique_committers.insert(identity);
                } else {
                    unique_committers.insert(format!("sha:{}", commit.sha));
                }
            }

            if commits.len() < 100 {
                break;
            }
            page += 1;
        }

        Ok(unique_committers.len() as i64)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = self.send(url).await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("GitHub API HTTP {}: {}", status.as_u16(), body));
        }
        response
            .json()
            .await
            .context("parse github response failed")
    }

    async fn send(&self, url: &str) -> Result<Response> {
        let mut request = self
            .client
            .get(url)
            .header(header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        request.send().await.context("send github request failed")
    }
}

fn normalized_committer_identity(commit: &GitHubCommitListItem) -> Option<String> {
    commit
        .commit
        .committer
        .as_ref()
        .and_then(|identity| identity.email.clone())
        .filter(|email| !email.trim().is_empty())
        .map(|email| email.to_ascii_lowercase())
        .or_else(|| {
            commit
                .commit
                .author
                .as_ref()
                .and_then(|identity| identity.email.clone())
                .filter(|email| !email.trim().is_empty())
                .map(|email| email.to_ascii_lowercase())
        })
        .or_else(|| {
            commit
                .commit
                .committer
                .as_ref()
                .and_then(|identity| identity.name.clone())
                .filter(|name| !name.trim().is_empty())
                .map(|name| format!("name:{}", name))
        })
        .or_else(|| {
            commit
                .commit
                .author
                .as_ref()
                .and_then(|identity| identity.name.clone())
                .filter(|name| !name.trim().is_empty())
                .map(|name| format!("name:{}", name))
        })
}

fn parse_github_repo(url: &str) -> Option<(String, String)> {
    let trimmed = url.trim().trim_end_matches('/').trim_end_matches(".git");

    let path = if trimmed.contains("://") {
        let parsed = Url::parse(trimmed).ok()?;
        let host = parsed.host_str()?;
        if host != "github.com" && host != "www.github.com" {
            return None;
        }
        parsed.path().trim_start_matches('/').to_string()
    } else if let Some(stripped) = trimmed.strip_prefix("github.com/") {
        stripped.to_string()
    } else if let Some(stripped) = trimmed.strip_prefix("www.github.com/") {
        stripped.to_string()
    } else {
        trimmed.to_string()
    };

    let mut segments = path.split('/').filter(|segment| !segment.is_empty());
    let owner = segments.next()?;
    let repo = segments.next()?;

    Some((owner.to_string(), repo.to_string()))
}

fn parse_last_page_from_link(link: &str) -> Option<u32> {
    for part in link.split(',') {
        let item = part.trim();
        if !item.contains("rel=\"last\"") {
            continue;
        }
        let start = item.find('<')?;
        let end = item.find('>')?;
        let url = &item[start + 1..end];
        let query = url.split('?').nth(1)?;
        for pair in query.split('&') {
            let mut iter = pair.splitn(2, '=');
            let key = iter.next()?;
            let value = iter.next().unwrap_or_default();
            if key == "page" {
                return value.parse::<u32>().ok();
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn package(repo_url: &str) -> packages::Model {
        let now = Utc::now();
        packages::Model {
            id: 1,
            name: "openssl".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: Some(repo_url.to_string()),
            description: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn commit_json(sha: &str, email: Option<&str>, name: Option<&str>, date: &str) -> Value {
        json!({
            "sha": sha,
            "commit": {
                "author": {
                    "name": name,
                    "email": email,
                    "date": date
                },
                "committer": {
                    "name": name,
                    "email": email,
                    "date": date
                }
            }
        })
    }

    #[test]
    fn parse_last_page_from_link_extracts_last_page() {
        let link = "<https://api.github.com/repositories/1/commits?per_page=1&page=2>; rel=\"next\", <https://api.github.com/repositories/1/commits?per_page=1&page=185>; rel=\"last\"";
        assert_eq!(parse_last_page_from_link(link), Some(185));
    }

    #[test]
    fn parse_last_page_from_link_returns_none_without_last() {
        let link =
            "<https://api.github.com/repositories/1/commits?per_page=1&page=2>; rel=\"next\"";
        assert_eq!(parse_last_page_from_link(link), None);
    }

    #[test]
    fn parse_github_repo_accepts_standard_variants() {
        assert_eq!(
            parse_github_repo("https://github.com/example/project"),
            Some(("example".to_string(), "project".to_string()))
        );
        assert_eq!(
            parse_github_repo("https://github.com/example/project.git"),
            Some(("example".to_string(), "project".to_string()))
        );
        assert_eq!(
            parse_github_repo("github.com/example/project"),
            Some(("example".to_string(), "project".to_string()))
        );
        assert_eq!(
            parse_github_repo("www.github.com/example/project/"),
            Some(("example".to_string(), "project".to_string()))
        );
        assert_eq!(
            parse_github_repo("example/project"),
            Some(("example".to_string(), "project".to_string()))
        );
    }

    #[test]
    fn parse_github_repo_rejects_non_github_url() {
        assert_eq!(
            parse_github_repo("https://sourceware.org/git/binutils-gdb.git"),
            None
        );
    }

    #[test]
    fn normalized_committer_identity_prefers_email_then_name_then_sha_fallback() {
        let with_committer_email = GitHubCommitListItem {
            sha: "a".to_string(),
            commit: GitHubCommitInfo {
                author: None,
                committer: Some(GitHubCommitIdentity {
                    name: Some("Committer".to_string()),
                    email: Some("USER@EXAMPLE.COM".to_string()),
                    date: None,
                }),
            },
        };
        assert_eq!(
            normalized_committer_identity(&with_committer_email).as_deref(),
            Some("user@example.com")
        );

        let with_author_name = GitHubCommitListItem {
            sha: "b".to_string(),
            commit: GitHubCommitInfo {
                author: Some(GitHubCommitIdentity {
                    name: Some("Author Name".to_string()),
                    email: Some("".to_string()),
                    date: None,
