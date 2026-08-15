use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::collectors::traits::{
    Branch, Commit, CommitStats, FileContent, Issue, IssueState, Repository,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomGitRepository {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub html_url: String,
    pub default_branch: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<AtomGitRepository> for Repository {
    fn from(repo: AtomGitRepository) -> Self {
        Self {
            id: repo.id,
            name: repo.name,
            full_name: repo.full_name,
            description: repo.description,
            html_url: repo.html_url,
            default_branch: repo.default_branch,
            created_at: repo
                .created_at
                .parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now()),
            updated_at: repo
                .updated_at
                .parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomGitBranch {
    pub name: String,
    pub commit: AtomGitCommitRef,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomGitCommitRef {
    pub sha: String,
}

impl From<AtomGitBranch> for Branch {
    fn from(branch: AtomGitBranch) -> Self {
        Self {
            name: branch.name,
            commit_sha: branch.commit.sha,
            protected: branch.protected,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomGitCommit {
    pub sha: String,
    pub commit: AtomGitCommitDetail,
    pub html_url: String,
    #[serde(default)]
    pub stats: Option<AtomGitCommitStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomGitCommitDetail {
    #[serde(default)]
    pub title: Option<String>,
    pub message: String,
    #[serde(default)]
    pub author: Option<AtomGitUser>,
    #[serde(default)]
    pub committer: Option<AtomGitUser>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AtomGitUser {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomGitCommitStats {
    pub additions: u32,
    pub deletions: u32,
    pub total: u32,
}

impl From<AtomGitCommit> for Commit {
    fn from(commit: AtomGitCommit) -> Self {
        let derived_title = commit
            .commit
            .title
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                commit
                    .commit
                    .message
                    .lines()
                    .next()
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default()
            });

        let author = commit
            .commit
            .author
            .clone()
            .or_else(|| commit.commit.committer.clone());
        let committer = commit
            .commit
            .committer
            .clone()
            .or_else(|| commit.commit.author.clone());
        let author_name = author
            .as_ref()
            .and_then(|u| u.name.as_ref())
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let author_email = author
            .as_ref()
            .and_then(|u| u.email.as_ref())
            .cloned()
            .unwrap_or_default();
        let author_date = author
            .as_ref()
            .and_then(|u| u.date.as_ref())
            .and_then(|d| d.parse::<DateTime<Utc>>().ok())
            .unwrap_or_else(Utc::now);
        let committer_name = committer
            .as_ref()
            .and_then(|u| u.name.as_ref())
            .cloned()
            .unwrap_or_else(|| author_name.clone());
        let committer_email = committer
            .as_ref()
            .and_then(|u| u.email.as_ref())
            .cloned()
            .unwrap_or_else(|| author_email.clone());
        let committer_date = committer
            .as_ref()
            .and_then(|u| u.date.as_ref())
            .and_then(|d| d.parse::<DateTime<Utc>>().ok())
            .unwrap_or(author_date);

        Self {
            sha: commit.sha,
            title: derived_title,
            message: commit.commit.message,
            author_name,
            author_email,
            author_date,
            committer_name,
            committer_email,
            committer_date,
            html_url: commit.html_url,
            stats: commit.stats.map(|s| CommitStats {
                additions: s.additions,
                deletions: s.deletions,
                total: s.total,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomGitFileContent {
    pub name: String,
    pub path: String,
    pub sha: String,
    pub size: u64,
    pub content: String,
    pub encoding: String,
    pub download_url: String,
}

impl From<AtomGitFileContent> for FileContent {
    fn from(file: AtomGitFileContent) -> Self {
        Self {
            name: file.name,
            path: file.path,
            sha: file.sha,
            size: file.size,
            content: file.content,
            encoding: file.encoding,
            download_url: file.download_url,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomGitError {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomGitIssue {
    pub number: i64,
    pub title: String,
    pub state: String,
    pub html_url: String,
    pub user: AtomGitIssueUser,
    #[serde(default)]
    pub labels: Vec<AtomGitIssueLabel>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomGitIssueUser {
    pub login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomGitIssueLabel {
    pub name: String,
}

impl From<AtomGitIssue> for Issue {
    fn from(issue: AtomGitIssue) -> Self {
        let raw = serde_json::to_value(&issue).unwrap_or(Value::Null);
        let created_at = issue
            .created_at
            .parse::<DateTime<Utc>>()
            .unwrap_or_else(|_| Utc::now());
        let updated_at = issue
            .updated_at
            .parse::<DateTime<Utc>>()
            .unwrap_or_else(|_| Utc::now());
        let closed_at = issue
            .finished_at
            .and_then(|ts| ts.parse::<DateTime<Utc>>().ok());
        let labels = issue.labels.into_iter().map(|l| l.name).collect();

        Issue {
            number: issue.number,
            title: issue.title,
            state: IssueState::parse_str(&issue.state),
            author: issue.user.login,
            api_url: issue.html_url,
            labels,
            created_at,
            updated_at,
            closed_at,
            raw_payload: raw,
        }
    }
}
