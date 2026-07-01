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
