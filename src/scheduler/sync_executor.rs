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
