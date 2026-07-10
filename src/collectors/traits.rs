/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. ctscat is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

use super::error::ApiResult;

/// 仓库信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub html_url: String,
    pub default_branch: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 分支信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub commit_sha: String,
    pub protected: bool,
}

/// Commit 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub title: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub author_date: DateTime<Utc>,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_date: DateTime<Utc>,
    pub html_url: String,
    pub stats: Option<CommitStats>,
}

/// Commit 统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitStats {
    pub additions: u32,
    pub deletions: u32,
    pub total: u32,
}

/// 文件内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    pub name: String,
    pub path: String,
    pub sha: String,
    pub size: u64,
    pub content: String,  // Base64 编码的内容
    pub encoding: String, // "base64"
    pub download_url: String,
}

/// 分页参数
#[derive(Debug, Clone)]
pub struct PaginationParams {
    pub page: u32,
    pub per_page: u32,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 30,
        }
    }
}

/// Issue 状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IssueState {
    Open,
    Closed,
    All,
}

impl IssueState {
    pub fn as_query_value(&self) -> &'static str {
        match self {
            IssueState::Open => "open",
            IssueState::Closed => "closed",
            IssueState::All => "all",
        }
    }

    pub fn parse_str(value: &str) -> Self {
        match value {
            "open" => IssueState::Open,
            "closed" => IssueState::Closed,
            _ => IssueState::All,
        }
    }
}

impl fmt::Display for IssueState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_query_value())
    }
}

/// Issue 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub number: i64,
    pub title: String,
    pub state: IssueState,
    pub author: String,
    pub api_url: String,
    pub labels: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub raw_payload: Value,
}

/// Issue 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueEvent {
    pub event_type: String,
    pub actor: Option<String>,
    pub occurred_at: DateTime<Utc>,
    pub raw_payload: Value,
}

/// Commits 查询参数
#[derive(Debug, Clone)]
pub struct CommitsParams {
    pub branch: String,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub page: u32,
    pub per_page: u32,
}

impl CommitsParams {
    pub fn new(branch: impl Into<String>) -> Self {
        Self {
            branch: branch.into(),
            since: None,
            until: None,
            page: 1,
            per_page: 30,
        }
    }

    pub fn since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    pub fn until(mut self, until: DateTime<Utc>) -> Self {
        self.until = Some(until);
        self
    }

    pub fn page(mut self, page: u32) -> Self {
        self.page = page;
        self
    }

    pub fn per_page(mut self, per_page: u32) -> Self {
        self.per_page = per_page;
        self
    }
}

/// Git 平台客户端通用接口
#[async_trait]
pub trait GitClient: Send + Sync {
    /// 获取仓库信息
    async fn get_repository(&self, owner: &str, repo: &str) -> ApiResult<Repository>;

    /// 获取分支列表
    async fn get_branches(&self, owner: &str, repo: &str) -> ApiResult<Vec<Branch>>;

    /// 获取指定分支的 commits
    async fn get_commits(
        &self,
        owner: &str,
        repo: &str,
        params: CommitsParams,
    ) -> ApiResult<Vec<Commit>>;

    /// 获取文件内容
    async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        branch: &str,
    ) -> ApiResult<FileContent>;

    /// 解码 Base64 内容
    fn decode_content(&self, content: &str) -> ApiResult<String> {
        use base64::{engine::general_purpose, Engine as _};

        // 移除换行符
        let cleaned = content.replace('\n', "");

        general_purpose::STANDARD
            .decode(cleaned.as_bytes())
            .map_err(|e| super::error::ApiError::Base64Error(e.to_string()))
            .and_then(|bytes| {
                String::from_utf8(bytes)
                    .map_err(|e| super::error::ApiError::Base64Error(e.to_string()))
            })
    }
}

/// Issues 客户端扩展接口
#[async_trait]
pub trait IssueClient: Send + Sync {
    async fn get_issues(
        &self,
        owner: &str,
        repo: &str,
        params: IssueParams,
    ) -> ApiResult<Vec<Issue>>;

    async fn get_issue_events(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: i64,
    ) -> ApiResult<Vec<IssueEvent>> {
        Ok(Vec::new())
    }
}

/// Issues 查询参数
#[derive(Debug, Clone)]
pub struct IssueParams {
    pub state: IssueState,
    pub page: u32,
    pub per_page: u32,
    pub since: Option<DateTime<Utc>>,
}

impl Default for IssueParams {
    fn default() -> Self {
        Self {
            state: IssueState::Open,
            page: 1,
            per_page: 20,
            since: None,
        }
    }
}

// ============================================================================
// 统一的 Collector 接口
// ============================================================================

use std::path::PathBuf;

/// 采集平台类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Platform {
    GitHub,
    GitLab,
    Gitee,
    Gitea,
    Local,
}

#[allow(clippy::should_implement_trait)]
impl Platform {
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::GitHub => "github",
            Platform::GitLab => "gitlab",
            Platform::Gitee => "gitee",
            Platform::Gitea => "gitea",
            Platform::Local => "local",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "github" => Some(Platform::GitHub),
            "gitlab" => Some(Platform::GitLab),
            "gitee" => Some(Platform::Gitee),
            "gitea" => Some(Platform::Gitea),
            "local" => Some(Platform::Local),
            _ => None,
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 采集器配置
#[derive(Debug, Clone)]
pub struct CollectConfig {
    /// 平台类型
    pub platform: Platform,
    /// 仓库所有者（远端平台需要）
    pub owner: Option<String>,
    /// 仓库名称（远端平台需要）
    pub repo: Option<String>,
    /// 本地仓库路径（local 平台需要）
    pub repo_path: Option<PathBuf>,
    /// 分支名称
    pub branch: String,
    /// API 地址（自建平台需要，如 Gitea）
    pub api_url: Option<String>,
    /// 认证 token
    pub token: Option<String>,
    /// 采集数量限制
    pub limit: Option<u32>,
    /// 起始时间（可选）
    pub since: Option<DateTime<Utc>>,
    /// 结束时间（可选）
    pub until: Option<DateTime<Utc>>,
    /// 采集层级（l0/l1/l2）
    pub level: Option<String>,
}

impl CollectConfig {
    /// 创建新的配置
    pub fn new(platform: Platform, branch: impl Into<String>) -> Self {
        Self {
            platform,
            owner: None,
            repo: None,
            repo_path: None,
            branch: branch.into(),
            api_url: None,
            token: None,
            limit: None,
            since: None,
            until: None,
            level: None,
        }
    }

    /// 设置远端仓库信息
    pub fn with_remote(mut self, owner: impl Into<String>, repo: impl Into<String>) -> Self {
        self.owner = Some(owner.into());
        self.repo = Some(repo.into());
        self
    }

    /// 设置本地仓库路径
    pub fn with_local_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.repo_path = Some(path.into());
        self
    }

    /// 设置 API 地址
    pub fn with_api_url(mut self, url: impl Into<String>) -> Self {
        self.api_url = Some(url.into());
        self
    }

    /// 设置认证 token
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// 设置采集数量限制
    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// 设置起始时间
    pub fn with_since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    /// 设置结束时间
    pub fn with_until(mut self, until: DateTime<Utc>) -> Self {
        self.until = Some(until);
        self
    }

    /// 设置采集层级
    pub fn with_level(mut self, level: impl Into<String>) -> Self {
        self.level = Some(level.into());
        self
    }
}

/// Commit 元数据（简化版，用于采集结果）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitMetadata {
    pub sha: String,
    /// 提交标题
    pub title: String,
    pub message: String,
    pub author: String,
    pub email: String,
    pub date: DateTime<Utc>,
    pub files_changed: Vec<String>,
}

impl From<Commit> for CommitMetadata {
    fn from(commit: Commit) -> Self {
        Self {
            sha: commit.sha,
            title: commit.title,
            message: commit.message,
            author: commit.author_name,
            email: commit.author_email,
            date: commit.author_date,
            files_changed: Vec::new(), // 需要额外获取
        }
    }
}

/// 快照数据（用于 L1/L2 采集）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotData {
    pub spec_content: Option<String>,
    pub spec_path: Option<String>,
    pub spec_version: Option<String>,
    pub spec_content_base64: Option<String>,
    pub patches: Vec<PatchFile>,
    pub spec_release: Option<String>,
    pub spec_sha256: Option<String>,
    pub source_files: Vec<SourceFile>,
    pub file_count: usize,
}

/// Patch 文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchFile {
    pub filename: String,
    pub path: String,
    pub content: String,
    pub sha256: String,
}

/// 源码文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFile {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

/// 采集结果
/// 文件信息（用于 L2 采集）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub is_binary: bool,
}

/// Spec 文件信息（用于 L2 采集）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecInfo {
    pub path: String,
    pub version: String,
    pub release: String,
    pub content_base64: String,
    pub sha256: String,
}

/// Issue 元数据（简化版）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueMetadata {
    pub number: i64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectResult {
    /// 层级（l0, l1, l2）
    pub level: String,
    /// 平台
    pub platform: String,
    /// 仓库所有者
    pub owner: Option<String>,
    /// 仓库名称
    pub repo: String,
    /// 分支名称
    pub branch: String,
    /// 采集时间
    pub collected_at: DateTime<Utc>,
    /// Commits 列表
    pub commits: Vec<CommitMetadata>,
    /// 快照数据（L1/L2 使用）
    pub snapshot: Option<SnapshotData>,
    /// 文件列表（L2 使用）
    pub files: Vec<FileInfo>,
    /// Spec 文件信息（L2 使用）
    pub spec: Option<SpecInfo>,
    /// Issues 列表（L2 使用）
    pub issues: Vec<IssueMetadata>,
}

/// 统一的采集器接口
#[async_trait]
pub trait Collector: Send + Sync {
    /// 采集元数据
    async fn collect(&self, config: &CollectConfig) -> ApiResult<CollectResult>;

    /// 获取采集器名称
    fn name(&self) -> &str;

    /// 检查配置是否有效
    fn validate_config(&self, config: &CollectConfig) -> ApiResult<()> {
        // 默认实现：检查基本配置
        match config.platform {
            Platform::Local => {
                if config.repo_path.is_none() {
                    return Err(super::error::ApiError::InvalidConfig(
                        "Local platform requires repo_path".to_string(),
                    ));
                }
            }
            _ => {
                if config.owner.is_none() || config.repo.is_none() {
                    return Err(super::error::ApiError::InvalidConfig(
                        "Remote platform requires owner and repo".to_string(),
                    ));
                }
            }
        }

        if config.branch.is_empty() {
            return Err(super::error::ApiError::InvalidConfig(
                "Branch name is required".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_from_str() {
        assert_eq!(Platform::from_str("github"), Some(Platform::GitHub));
        assert_eq!(Platform::from_str("Github"), Some(Platform::GitHub));
        assert_eq!(Platform::from_str("gitlab"), Some(Platform::GitLab));
        assert_eq!(Platform::from_str("gitee"), Some(Platform::Gitee));
        assert_eq!(Platform::from_str("gitea"), Some(Platform::Gitea));
        assert_eq!(Platform::from_str("local"), Some(Platform::Local));
        assert_eq!(Platform::from_str("unknown"), None);
    }

    #[test]
    fn test_platform_display() {
        assert_eq!(format!("{}", Platform::GitHub), "github");
        assert_eq!(format!("{}", Platform::GitLab), "gitlab");
        assert_eq!(format!("{}", Platform::Gitee), "gitee");
        assert_eq!(format!("{}", Platform::Gitea), "gitea");
        assert_eq!(format!("{}", Platform::Local), "local");
    }

    #[test]
    fn test_pagination_params_default() {
        let params = PaginationParams::default();
        assert_eq!(params.page, 1);
        assert_eq!(params.per_page, 30);
    }

    #[test]
    fn test_issue_state() {
        assert_eq!(IssueState::Open.as_query_value(), "open");
        assert_eq!(IssueState::Closed.as_query_value(), "closed");
        assert_eq!(IssueState::All.as_query_value(), "all");

        assert_eq!(IssueState::parse_str("open"), IssueState::Open);
        assert_eq!(IssueState::parse_str("closed"), IssueState::Closed);
        assert_eq!(IssueState::parse_str("other"), IssueState::All);

        assert_eq!(format!("{}", IssueState::Open), "open");
    }

    #[test]
    fn test_commits_params() {
        let params = CommitsParams::new("main")
            .page(2)
            .per_page(50)
            .since(Utc::now())
            .until(Utc::now());

        assert_eq!(params.branch, "main");
        assert_eq!(params.page, 2);
        assert_eq!(params.per_page, 50);
        assert!(params.since.is_some());
        assert!(params.until.is_some());
    }

    #[test]
    fn test_issue_params_default() {
        let params = IssueParams::default();
        assert_eq!(params.state, IssueState::Open);
        assert_eq!(params.page, 1);
        assert_eq!(params.per_page, 20);
        assert!(params.since.is_none());
    }

    #[test]
    fn test_collect_config_builder() {
        let config = CollectConfig::new(Platform::GitHub, "main")
            .with_remote("owner", "repo")
            .with_token("token")
            .with_api_url("url")
            .with_limit(10)
            .with_level("l2")
            .with_since(Utc::now())
            .with_until(Utc::now())
            .with_local_path("/path");

        assert_eq!(config.platform, Platform::GitHub);
        assert_eq!(config.branch, "main");
        assert_eq!(config.owner, Some("owner".to_string()));
        assert_eq!(config.repo, Some("repo".to_string()));
        assert_eq!(config.token, Some("token".to_string()));
        assert_eq!(config.api_url, Some("url".to_string()));
        assert_eq!(config.limit, Some(10));
        assert_eq!(config.level, Some("l2".to_string()));
        assert!(config.since.is_some());
        assert!(config.until.is_some());
        assert!(config.repo_path.is_some());
    }

    struct TestCollector;

    #[async_trait]
    impl Collector for TestCollector {
        async fn collect(&self, _config: &CollectConfig) -> ApiResult<CollectResult> {
            unimplemented!()
        }

        fn name(&self) -> &str {
            "TestCollector"
        }
    }

    #[test]
    fn test_validate_config() {
        let collector = TestCollector;

        // Test Remote platform validation
        let config = CollectConfig::new(Platform::GitHub, "main");
        assert!(collector.validate_config(&config).is_err()); // Missing owner/repo

        let config = CollectConfig::new(Platform::GitHub, "main").with_remote("owner", "repo");
        assert!(collector.validate_config(&config).is_ok());

        // Test Local platform validation
        let config = CollectConfig::new(Platform::Local, "main");
        assert!(collector.validate_config(&config).is_err()); // Missing repo_path

        let config = CollectConfig::new(Platform::Local, "main").with_local_path("/path");
        assert!(collector.validate_config(&config).is_ok());

        // Test Empty branch
        let config = CollectConfig::new(Platform::GitHub, "").with_remote("owner", "repo");
        assert!(collector.validate_config(&config).is_err());
    }
}
