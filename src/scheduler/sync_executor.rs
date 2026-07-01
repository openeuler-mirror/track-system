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

//! 同步任务执行器
//!
//! 负责实际执行同步任务，将API客户端、数据抓取服务和任务管理器连接起来

use std::sync::Arc;

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use tracing::{error, info, warn};

use crate::{
    collectors::{GitClient, IssueClient},
    entities::tracking,
    telemetry::Telemetry,
};

use super::{SyncManager, SyncResult, SyncService, SyncStatus};

const DEFAULT_MAX_TASKS: usize = 4;

#[derive(Debug, Default, Clone)]
pub struct SyncExecutionStats {
    pub discovered: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub errors: Vec<(i32, String)>,
}

impl SyncExecutionStats {
    fn record_outcome(&mut self, tracking_id: i32, outcome: &SyncResult) {
        self.processed += 1;
        match outcome.status {
            SyncStatus::Success => self.succeeded += 1,
            SyncStatus::Skipped => self.skipped += 1,
            SyncStatus::Failed => {
                self.failed += 1;
                self.errors.push((tracking_id, outcome.message.clone()));
            }
        }
    }

    fn record_error(&mut self, tracking_id: i32, message: String) {
        self.processed += 1;
        self.failed += 1;
        self.errors.push((tracking_id, message));
    }
}

pub trait SyncApiClient: GitClient + IssueClient + Send + Sync {}
impl<T> SyncApiClient for T where T: GitClient + IssueClient + Send + Sync {}

/// 同步执行器
#[allow(dead_code)]
pub struct SyncExecutor<'a> {
    db: &'a DatabaseConnection,
    client: Option<Arc<dyn SyncApiClient>>,
    sync_manager: SyncManager<'a>,
}

impl<'a> SyncExecutor<'a> {
    /// 创建新的同步执行器
    pub fn new(db: &'a DatabaseConnection, client: Option<Arc<dyn SyncApiClient>>) -> Self {
        Self {
            db,
            client,
            sync_manager: SyncManager::new(db),
        }
    }

    /// 执行单个tracking的同步任务
    pub async fn execute_sync(&self, tracking_id: i32) -> Result<SyncResult> {
        info!(tracking_id = tracking_id, "开始执行同步任务");

        // 启动同步任务
        self.sync_manager
            .start_sync_task(tracking_id)
            .await
            .context("启动同步任务失败")?;

        // 获取tracking配置
        let tracking = self
