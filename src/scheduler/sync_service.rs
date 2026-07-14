/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

//! 同步服务实现 - 负责实际的 L1 到数据库的数据拉取

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tracing::{debug, info, warn};

// 辅助函数：判断是否为 openeuler-ci-bot 提交
fn is_openeuler_ci_bot(author: &str, email: &str) -> bool {
    let a = author.trim().to_ascii_lowercase();
    let e = email.trim().to_ascii_lowercase();
    a == "openeuler-ci-bot" || e.contains("openeuler-ci-bot") || e.contains("ci-bot")
}

use crate::collectors::traits::GitClient;
use crate::collectors::traits::{
    CollectConfig, Collector, IssueClient, IssueParams, IssueState, Platform,
};
use crate::entities::{l1_commit_records, prelude::*, tracking};

/// 同步服务
pub struct SyncService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> SyncService<'a> {
    /// 创建新的同步服务
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// 同步指定 tracking 的数据
    ///
    /// 根据 tracking 配置自动选择合适的 Collector 进行数据采集
    pub async fn sync_tracking(&self, tracking_id: i32) -> Result<SyncResult> {
        debug!(tracking_id = tracking_id, "开始同步 tracking");

        // 1. 查询 tracking 配置
        let tracking_entity = Tracking::find_by_id(tracking_id)
            .one(self.db)
            .await
            .context("查询 tracking 失败")?
            .ok_or_else(|| anyhow::anyhow!("Tracking {} 不存在", tracking_id))?;

        // 检查同步状态：仅对暂停/归档任务跳过
        if matches!(
            tracking_entity.tracking_status.as_str(),
            "paused" | "archived"
        ) {
            warn!(
                tracking_id = tracking_id,
                status = %tracking_entity.tracking_status,
                "Tracking 已暂停或归档"
            );
            return Ok(SyncResult::skipped("Tracking 未处于可同步状态"));
        }

        // 2. 确定平台类型
        // 目前从环境变量或 repo_owner 推断平台
        let platform = self.infer_platform(&tracking_entity)?;

        // 3. 获取认证 token
        let token = self.get_platform_token(&platform)?;

        // 4. 创建 Collector
        let collector = self.create_collector(platform, token)?;

        // 5. 使用 Collector 进行同步
        self.sync_tracking_with_collector(tracking_id, collector.as_ref())
            .await
    }

    /// 推断平台类型
    ///
    fn infer_platform(&self, tracking: &tracking::Model) -> Result<Platform> {
        // 优先从环境变量读取
        if let Ok(platform_str) = std::env::var("DEFAULT_PLATFORM") {
            info!(default_platform = %platform_str, "从环境变量 DEFAULT_PLATFORM 读取平台");
            if let Some(platform) = Platform::from_str(&platform_str) {
                return Ok(platform);
            }
        }

        // 根据 repo_owner 推断（简单启发式）

        debug!(tracking_id = tracking.id, platform = %tracking.platform.as_deref().unwrap_or("Unknown"), "根据 repo_owner 推断平台");
        if tracking.platform.as_deref() == Some("github") {
            Ok(Platform::GitHub)
        } else if tracking.platform.as_deref() == Some("gitea") {
            Ok(Platform::Gitea)
        } else if tracking.platform.as_deref() == Some("atomgit") {
            Ok(Platform::AtomGit)
        } else {
            // 默认使用 Gitee（当前系统主要使用的平台）
            Ok(Platform::Gitee)
        }
    }

    /// 获取平台对应的认证 token
    fn get_platform_token(&self, platform: &Platform) -> Result<Option<String>> {
        let env_var = match platform {
            Platform::GitHub => "GITHUB_TOKEN",
            Platform::AtomGit => "ATOMGIT_TOKEN",
            Platform::Gitee => "GITEE_TOKEN",
            Platform::Gitea => "GITEA_TOKEN",
            Platform::GitLab => "GITLAB_TOKEN",
            Platform::Local => return Ok(None), // Local 不需要 token
        };

        match std::env::var(env_var) {
            Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
            _ => {
                warn!(platform = %platform, "缺少 {} 环境变量", env_var);
                Ok(None)
            }
        }
    }

    /// 创建对应平台的 Collector
    fn create_collector(
        &self,
        platform: Platform,
        token: Option<String>,
    ) -> Result<Box<dyn Collector>> {
        use crate::collectors::{AtomGitClient, GitHubClient, GiteaClient, GiteeClient};

        match platform {
            Platform::GitHub => {
                let token = token.ok_or_else(|| anyhow::anyhow!("GitHub 需要 token"))?;
                let client = GitHubClient::new(token)?;
                let collector = client.as_collector();
                Ok(Box::new(collector))
            }
            Platform::AtomGit => {
                let token = token.ok_or_else(|| anyhow::anyhow!("AtomGit 需要 token"))?;
                let client = AtomGitClient::new(token, "master")?;
                let collector = client.as_collector();
                Ok(Box::new(collector))
            }
            Platform::Gitee => {
                let token = token.ok_or_else(|| anyhow::anyhow!("Gitee 需要 token"))?;
                let client = GiteeClient::new(token)?;
                let collector = client.as_collector();
                Ok(Box::new(collector))
            }
            Platform::Gitea => {
                let token = token.ok_or_else(|| anyhow::anyhow!("Gitea 需要 token"))?;
                // Gitea 需要 API URL，从环境变量获取
                let api_url = std::env::var("GITEA_API_URL")
                    .unwrap_or_else(|_| "https://gitea.com".to_string());
                let client = GiteaClient::new(token, api_url)?;
                let collector = client.as_collector();
                Ok(Box::new(collector))
            }
            Platform::GitLab => {
                // TODO: 实现 GitLab Collector
                Err(anyhow::anyhow!("GitLab Collector 尚未实现"))
            }
            Platform::Local => {
                // TODO: 实现 Local Collector
                Err(anyhow::anyhow!("Local Collector 尚未实现"))
            }
        }
    }

    /// 使用 Collector 同步指定 tracking
    pub async fn sync_tracking_with_collector(
        &self,
        tracking_id: i32,
        collector: &dyn Collector,
    ) -> Result<SyncResult> {
        debug!(
            tracking_id = tracking_id,
            collector = collector.name(),
            "使用 Collector 同步数据"
        );

        // 1. 查询 tracking 配置
        let tracking_entity = Tracking::find_by_id(tracking_id)
            .one(self.db)
            .await
            .context("查询 tracking 失败")?
            .ok_or_else(|| anyhow::anyhow!("Tracking {} 不存在", tracking_id))?;

        // 2. 构建采集配置
        let platform = self.infer_platform(&tracking_entity)?;

        debug!(
            tracking_id = tracking_id,
            platform = %platform,
            l1_branch = %tracking_entity.l1_branch,
            l1_repo_owner = %tracking_entity.l1_repo_owner,
            l1_repo_name = %tracking_entity.l1_repo_name,
            "构建采集配置"
        );
        let config = CollectConfig::new(platform, &tracking_entity.l1_branch).with_remote(
            &tracking_entity.l1_repo_owner,
            &tracking_entity.l1_repo_name,
        );

        // 3. 验证配置
        collector
            .validate_config(&config)
            .context("Collector 配置验证失败")?;

        // 4. 执行采集
        let collect_result = collector.collect(&config).await.context("采集数据失败")?;

        debug!(
            tracking_id = tracking_id,
            commits_count = collect_result.commits.len(),
            "采集完成"
        );

        // 5. 保存采集结果到数据库
        let commits_synced = self
            .save_commits(
                tracking_id,
                &collect_result.commits,
                &tracking_entity,
                platform,
            )
            .await?;

        // 6. 同步 issues（如果 Collector 支持）
        let issues_synced = self
            .sync_issues_from_collector(tracking_id, collector, &tracking_entity)
            .await?;

        // 7. 更新 tracking 的同步时间
        self.update_tracking_sync_time(tracking_id).await?;

        info!(
            tracking_id = tracking_id,
            commits = commits_synced,
            issues = issues_synced,
            "同步完成"
        );

        Ok(SyncResult::success(commits_synced, issues_synced))
    }

    /// 保存 commits 到数据库
    async fn save_commits(
        &self,
        tracking_id: i32,
        commits: &[crate::collectors::traits::CommitMetadata],
        tracking: &tracking::Model,
        platform: Platform,
    ) -> Result<usize> {
        debug!(platform = %platform, "保存 commits 到数据库");
        match platform {
            Platform::Gitee | Platform::AtomGit => {
                self.process_platform_commits(tracking_id, commits, tracking, platform)
                    .await
            }
            _ => {
                info!("暂不支持同步 {} 平台的 commits", platform);
                Ok(0)
            }
        }
    }

    async fn process_platform_commits(
        &self,
        tracking_id: i32,
        commits: &[crate::collectors::traits::CommitMetadata],
        tracking: &tracking::Model,
        platform: Platform,
    ) -> Result<usize> {
        use std::collections::HashSet;
        let mut saved_count = 0;
        let mut commits_sorted = commits.to_vec();
        commits_sorted.sort_by_key(|c| c.date);
        let mut seen_version_release: HashSet<(String, String)> = HashSet::new();

        for commit in &commits_sorted {
            let existing = L1CommitRecords::find()
                .filter(l1_commit_records::Column::TrackingId.eq(tracking_id))
                .filter(l1_commit_records::Column::CommitSha.eq(&commit.sha))
                .one(self.db)
                .await?;

            if existing.is_some() {
                continue;
            }

            let api_url = match platform {
                Platform::Gitee => format!(
                    "https://gitee.com/src-openeuler/{}/commit/{}",
                    tracking.l1_repo_name, commit.sha
                ),
                Platform::AtomGit => format!(
                    "https://atomgit.com/src-openeuler/{}/commit/{}",
                    tracking.l1_repo_name, commit.sha
                ),
                _ => String::new(),
            };
            let (spec_version_opt, spec_release_opt) = self
                .fetch_spec_version_release(platform, tracking, &commit.sha)
                .await;

            if let Some(ver) = spec_version_opt.as_ref() {
                let rel_key = spec_release_opt.clone().unwrap_or_default();
                let key = (ver.clone(), rel_key.clone());
                let is_bot = is_openeuler_ci_bot(&commit.author, &commit.email);
                if seen_version_release.contains(&key) && is_bot {
                    continue;
                }
                if !seen_version_release.contains(&key) {
                    seen_version_release.insert(key);
                }
            }

            let new_commit = l1_commit_records::ActiveModel {
                tracking_id: Set(tracking_id),
                commit_sha: Set(commit.sha.clone()),
                commit_message: Set(commit.message.clone()),
                author_name: Set(commit.author.clone()),
                author_email: Set(commit.email.clone()),
                committed_at: Set(commit.date),
                change_type: Set(None),
                primary_change_type: Set(None),
                cve_list: Set(None),
                spec_changed: Set(false),
                patch_stats: Set(None),
                classification_status: Set("pending".to_string()),
                classification_notes: Set(None),
                sync_status: Set("synced".to_string()),
                synced_to_l2_commit: Set(None),
                synced_at: Set(None),
                api_url: Set(api_url),
                fetched_at: Set(Utc::now()),
                files_changed_count: Set(commit.files_changed.len() as i32),
                additions: Set(0),
                deletions: Set(0),
                created_at: Set(Utc::now()),
                updated_at: Set(Utc::now()),
                spec_version: Set(spec_version_opt),
                spec_release: Set(spec_release_opt),
                ..Default::default()
            };

            new_commit
                .insert(self.db)
                .await
                .context("保存 commit 失败")?;

            saved_count += 1;
        }

        info!(
            saved = saved_count,
            platform = %platform,
            "保存了 {} 个新 commits",
            saved_count
        );
        Ok(saved_count)
    }

    async fn fetch_spec_version_release(
        &self,
        platform: Platform,
        tracking: &tracking::Model,
        commit_sha: &str,
    ) -> (Option<String>, Option<String>) {
        use crate::component::normalize_spec_path;
        use crate::spec::parse_spec;

        let mut spec_version: Option<String> = None;
        let mut spec_release: Option<String> = None;

        match platform {
            Platform::Gitee => {
                use crate::collectors::gitee::GiteeClient;
                if let Ok(token) = std::env::var("GITEE_TOKEN") {
                    if !token.trim().is_empty() {
                        let client = GiteeClient::new(token).ok();
                        if let Some(client) = client {
                            let spec_path = normalize_spec_path(&tracking.l1_repo_name, None);
                            if let Ok(file) = client
                                .get_file_content(
                                    &tracking.l1_repo_owner,
                                    &tracking.l1_repo_name,
                                    &spec_path,
                                    commit_sha,
                                )
                                .await
                            {
                                if let Ok(decoded) = client.decode_content(&file.content) {
                                    let info = parse_spec(&decoded);
                                    if !info.version.is_empty() {
                                        spec_version = Some(info.version);
                                    }
                                    if !info.release.is_empty() {
                                        spec_release = Some(info.release);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Platform::AtomGit => {
                use crate::collectors::atomgit::AtomGitClient;
                if let Ok(token) = std::env::var("ATOMGIT_TOKEN") {
                    if !token.trim().is_empty() {
                        let client = AtomGitClient::new(token, tracking.l1_branch.clone()).ok();
                        if let Some(client) = client {
                            let spec_path = normalize_spec_path(&tracking.l1_repo_name, None);
                            if let Ok(file) = client
                                .get_file_content(
                                    &tracking.l1_repo_owner,
                                    &tracking.l1_repo_name,
                                    &spec_path,
                                    commit_sha,
                                )
                                .await
                            {
                                if let Ok(decoded) = client.decode_content(&file.content) {
                                    let info = parse_spec(&decoded);
                                    if !info.version.is_empty() {
                                        spec_version = Some(info.version);
                                    }
                                    if !info.release.is_empty() {
                                        spec_release = Some(info.release);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        (spec_version, spec_release)
    }

    /// 从 Collector 同步 issues
    async fn sync_issues_from_collector(
        &self,
        tracking_id: i32,
        _collector: &dyn Collector,
        _tracking: &tracking::Model,
    ) -> Result<usize> {
        // 尝试将 Collector 转换为 IssueClient
        // 注意：这里需要 Collector 同时实现 IssueClient trait
        // 由于 Rust 的 trait object 限制，我们需要通过其他方式处理
        // 暂时保留原有的 IssueClient 逻辑

        // TODO: 重构 IssueClient 集成到 Collector 中
        warn!(
            tracking_id = tracking_id,
            "Issues 同步暂时跳过,需要重构 IssueClient 集成"
        );

        // 使用旧的 sync_issues 方法
        // 这里需要创建对应的客户端
        Ok(0)
    }

    /// 同步 issues
    #[allow(unused)]
    async fn sync_issues<C>(
        &self,
        client: &C,
        tracking_id: i32,
        owner: &str,
        repo: &str,
    ) -> Result<usize>
    where
        C: IssueClient + Send + Sync + ?Sized,
    {
        info!("同步 issues");
        use crate::entities::{issues, prelude::*};
        use sea_orm::Set;

        // 查询所有 open issues
        let params = IssueParams {
            state: IssueState::All,
            per_page: 100,
            ..Default::default()
        };

        let api_issues = client
            .get_issues(owner, repo, params)
            .await
            .context("获取 issues 失败")?;

        info!("获取到 {} 个 issues", api_issues.len());

        let mut synced_count = 0;

        for api_issue in api_issues {
            // 转换 number 为 String
            let issue_number_str = api_issue.number.to_string();

            // 检查是否已存在
            use sea_orm::ColumnTrait;
            use sea_orm::QueryFilter;

            let existing = Issues::find()
                .filter(issues::Column::TrackingId.eq(tracking_id))
                .filter(issues::Column::IssueNumber.eq(&issue_number_str))
                .one(self.db)
                .await
                .context("查询已有 issue 失败")?;

            if existing.is_some() {
                // 已存在，跳过
                continue;
            }

            // 转换 labels 为 JsonValue
            let labels_json = if api_issue.labels.is_empty() {
                None
            } else {
                Some(serde_json::to_value(&api_issue.labels).unwrap_or(serde_json::Value::Null))
            };

            // 插入新记录
            let new_issue = issues::ActiveModel {
                tracking_id: Set(tracking_id),
                issue_number: Set(issue_number_str),
                title: Set(api_issue.title),
                state: Set(api_issue.state.to_string()),
                author: Set(api_issue.author),
                api_url: Set(api_issue.api_url),
                labels: Set(labels_json),
                created_at: Set(api_issue.created_at),
                updated_at: Set(api_issue.updated_at),
                closed_at: Set(api_issue.closed_at),
                raw_payload: Set(Some(api_issue.raw_payload)),
                ..Default::default()
            };

            Issues::insert(new_issue)
                .exec(self.db)
                .await
                .context("插入 issue 失败")?;

            synced_count += 1;
        }

        info!("新增 {} 个 issues", synced_count);
        Ok(synced_count)
    }

    /// 更新 tracking 的同步时间
    async fn update_tracking_sync_time(&self, tracking_id: i32) -> Result<()> {
        let tracking_entity = Tracking::find_by_id(tracking_id)
            .one(self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Tracking 不存在"))?;

        let mut active_model: tracking::ActiveModel = tracking_entity.into();
        // 使用正确的字段名
        active_model.last_sync_time = Set(Some(Utc::now()));
        active_model.tracking_status = Set("active".to_string());
        active_model.last_error = Set(None);
        active_model.updated_at = Set(Utc::now());

        active_model.update(self.db).await?;
        Ok(())
    }
}

/// 同步结果状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStatus {
    Success,
    Skipped,
    Failed,
}

/// 同步结果
#[derive(Debug, Clone)]
pub struct SyncResult {
    pub status: SyncStatus,
    pub commits_synced: usize,
    pub issues_synced: usize,
    pub message: String,
}

impl SyncResult {
    pub fn success(commits: usize, issues: usize) -> Self {
        Self {
            status: SyncStatus::Success,
            commits_synced: commits,
            issues_synced: issues,
            message: "同步成功".to_string(),
        }
    }

    pub fn skipped(reason: &str) -> Self {
        Self {
            status: SyncStatus::Skipped,
            commits_synced: 0,
            issues_synced: 0,
            message: reason.to_string(),
        }
    }

    pub fn failed(reason: &str) -> Self {
        Self {
            status: SyncStatus::Failed,
            commits_synced: 0,
            issues_synced: 0,
            message: reason.to_string(),
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self.status, SyncStatus::Success)
    }

    pub fn is_skipped(&self) -> bool {
        matches!(self.status, SyncStatus::Skipped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_openeuler_ci_bot_by_name() {
        assert!(is_openeuler_ci_bot("openeuler-ci-bot", "user@example.com"));
        assert!(is_openeuler_ci_bot("OpenEuler-CI-Bot", "user@example.com"));
        assert!(is_openeuler_ci_bot("OPENEULER-CI-BOT", "user@example.com"));
    }

    #[test]
    fn test_is_openeuler_ci_bot_by_email() {
        assert!(is_openeuler_ci_bot("User", "openeuler-ci-bot@example.com"));
        assert!(is_openeuler_ci_bot("User", "ci-bot@example.com"));
        assert!(is_openeuler_ci_bot("User", "CI-BOT@example.com"));
    }

    #[test]
    fn test_is_openeuler_ci_bot_false() {
        assert!(!is_openeuler_ci_bot("John Doe", "john@example.com"));
        assert!(!is_openeuler_ci_bot("Developer", "dev@company.com"));
    }

    #[test]
    fn test_sync_status_enum() {
        let success = SyncStatus::Success;
        let skipped = SyncStatus::Skipped;
        let failed = SyncStatus::Failed;

        assert_ne!(success, skipped);
        assert_ne!(success, failed);
        assert_ne!(skipped, failed);
    }

    #[test]
    fn test_sync_result_success() {
        let result = SyncResult::success(10, 5);

        assert_eq!(result.status, SyncStatus::Success);
        assert_eq!(result.commits_synced, 10);
        assert_eq!(result.issues_synced, 5);
        assert_eq!(result.message, "同步成功");
        assert!(result.is_success());
        assert!(!result.is_skipped());
    }

    #[test]
    fn test_sync_result_skipped() {
        let result = SyncResult::skipped("Tracking未激活");

        assert_eq!(result.status, SyncStatus::Skipped);
        assert_eq!(result.commits_synced, 0);
        assert_eq!(result.issues_synced, 0);
        assert_eq!(result.message, "Tracking未激活");
        assert!(!result.is_success());
        assert!(result.is_skipped());
    }

    #[test]
    fn test_sync_result_failed() {
        let result = SyncResult::failed("网络错误");

        assert_eq!(result.status, SyncStatus::Failed);
        assert_eq!(result.commits_synced, 0);
        assert_eq!(result.issues_synced, 0);
        assert_eq!(result.message, "网络错误");
        assert!(!result.is_success());
        assert!(!result.is_skipped());
    }

    #[test]
    fn test_sync_result_clone() {
        let result = SyncResult::success(3, 2);
        let cloned = result.clone();

        assert_eq!(result.status, cloned.status);
        assert_eq!(result.commits_synced, cloned.commits_synced);
        assert_eq!(result.issues_synced, cloned.issues_synced);
        assert_eq!(result.message, cloned.message);
    }

    #[tokio::test]
    async fn test_sync_tracking_not_found() {
        use sea_orm::{DatabaseBackend, MockDatabase};

        // Mock empty result for find_by_id
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<crate::entities::tracking::Model, _, _>(vec![vec![]])
            .into_connection();

        let service = SyncService::new(&db);
        let result = service.sync_tracking(1).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Tracking 1 不存在");
    }

    #[tokio::test]
    async fn test_sync_tracking_paused_skips() {
        use crate::entities::tracking;
        use chrono::Utc;
        use sea_orm::{DatabaseBackend, MockDatabase};

        let tracking_model = tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: "paused".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
            platform: Some("Gitee".to_string()),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results(vec![vec![tracking_model]])
            .into_connection();

        let service = SyncService::new(&db);
        let result = service.sync_tracking(1).await;

        assert!(result.is_ok());
        let sync_result = result.unwrap();
        assert!(sync_result.is_skipped());
        assert_eq!(sync_result.message, "Tracking 未处于可同步状态");
    }
}
