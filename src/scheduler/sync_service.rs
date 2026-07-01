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
