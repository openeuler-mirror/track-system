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

//! Cron调度器
//!
//! 负责根据配置的间隔定期执行同步和分类任务

use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::time::Duration;
use tracing::info;

use super::SyncManager;

/// Cron调度器
pub struct CronScheduler<'a> {
    #[allow(dead_code)]
    db: &'a DatabaseConnection,
    sync_manager: SyncManager<'a>,
}

impl<'a> CronScheduler<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self {
            db,
            sync_manager: SyncManager::new(db),
        }
    }

