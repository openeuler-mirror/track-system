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

//! 调度器管理器
//!
//! 负责管理所有同步任务的调度和执行

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info};

use super::{PipelineExecutor, SyncApiClient, SyncJobResult, SyncManager};

/// 调度器配置
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub max_concurrent_jobs: usize,
    pub job_timeout_secs: u64,
    pub cleanup_interval_secs: u64,
    pub health_check_interval_secs: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 10,
            job_timeout_secs: 1800,         // 30 分钟
            cleanup_interval_secs: 3600,    // 1 小时
            health_check_interval_secs: 30, // 30 秒
        }
    }
}
