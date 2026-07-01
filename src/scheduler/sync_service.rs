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

//! 同步服务实现 - 负责实际的 L1 到数据库的数据拉取

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tracing::{info, warn};

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
        info!(tracking_id = tracking_id, "开始同步 tracking");

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
            if let Some(platform) = Platform::from_str(&platform_str) {
                return Ok(platform);
            }
        }

        // 根据 repo_owner 推断（简单启发式）
        // 这只是临时方案
        if tracking.l1_repo_owner.contains("github") {
            Ok(Platform::GitHub)
        } else if tracking.l1_repo_owner.contains("gitea") {
            Ok(Platform::Gitea)
        } else {
            // 默认使用 Gitee（当前系统主要使用的平台）
            Ok(Platform::Gitee)
        }
    }

    /// 获取平台对应的认证 token
    fn get_platform_token(&self, platform: &Platform) -> Result<Option<String>> {
        let env_var = match platform {
            Platform::GitHub => "GITHUB_TOKEN",
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
        use crate::collectors::{GitHubClient, GiteaClient, GiteeClient};

        match platform {
            Platform::GitHub => {
                let token = token.ok_or_else(|| anyhow::anyhow!("GitHub 需要 token"))?;
                let client = GitHubClient::new(token)?;
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
        info!(
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

        info!(
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

        info!(
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
        use std::collections::HashSet;
        let mut saved_count = 0;

        info!(platform = %platform, "保存 commits 到数据库");
        if platform == Platform::Gitee {
            // 为确保作者提交优先于机器人提交，按时间升序处理
            let mut commits_sorted = commits.to_vec();
            commits_sorted.sort_by_key(|c| c.date);
            let mut seen_version_release: HashSet<(String, String)> = HashSet::new();

            for commit in &commits_sorted {
                // 检查是否已存在
                let existing = L1CommitRecords::find()
                    .filter(l1_commit_records::Column::TrackingId.eq(tracking_id))
                    .filter(l1_commit_records::Column::CommitSha.eq(&commit.sha))
                    .one(self.db)
                    .await?;

                if existing.is_some() {
                    continue; // 跳过已存在的
                }

                // 构造 API URL gitee 默认使用src-openeuler/ 前缀
                // 根据实际平台构造正确的 API URL

                let api_url = format!(
                    "https://gitee.com/src-openeuler/{}/commit/{}",
                    tracking.l1_repo_name, commit.sha
                );

                // 解析 spec 版本与 release：按该 commit 的 SHA 拉取 spec 文件内容
                // 仅在平台为 Gitee 且存在 repo 信息时进行
                let (spec_version_opt, spec_release_opt) = {
                    use crate::collectors::gitee::GiteeClient;
                    use crate::component::normalize_spec_path;
                    use crate::spec::parse_spec;
                    let mut spec_version: Option<String> = None;
                    let mut spec_release: Option<String> = None;

                    // 尝试通过 Gitee API 获取该 commit 的 spec 内容
                    // URL 形如：/contents/<spec_path>?ref=<commitsha>
                    // 我们复用 GiteeClient::get_file_content，并将 branch 参数传入 commit.sha
                    if let Ok(token) = std::env::var("GITEE_TOKEN") {
                        if !token.trim().is_empty() {
                            // 构建客户端
                            let client = GiteeClient::new(token).ok();
                            if let Some(client) = client {
                                let spec_path = normalize_spec_path(&tracking.l1_repo_name, None);
                                match client
                                    .get_file_content(
                                        &tracking.l1_repo_owner,
                                        &tracking.l1_repo_name,
                                        &spec_path,
                                        &commit.sha,
                                    )
                                    .await
                                {
                                    Ok(file) => {
                                        // Base64 解码并解析
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
                                    Err(_e) => {
                                        // 拉取失败则忽略，不阻塞提交保存
                                    }
                                }
                            }
                        }
                    }

                    (spec_version, spec_release)
                };

                // 如果该版本-发布组合已被非机器人提交占用，则跳过机器人的重复提交
                if let Some(ver) = spec_version_opt.as_ref() {
                    let rel_key = spec_release_opt.clone().unwrap_or_default();
                    let key = (ver.clone(), rel_key.clone());
                    let is_bot = is_openeuler_ci_bot(&commit.author, &commit.email);
