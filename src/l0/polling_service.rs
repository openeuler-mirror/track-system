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

//! L0仓库轮询服务
//!
//! 负责定期从L0（上游社区）仓库轮询新commit并检测差异

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tracing::{debug, info};

use crate::collectors::traits::{CollectConfig, Collector, Platform};
use crate::entities::l0_commits;

/// L0轮询摘要
#[derive(Debug, Clone)]
pub struct L0PollingResult {
    /// 拉取时间
    pub polled_at: chrono::DateTime<chrono::Utc>,
    /// 新发现的commit数
    pub new_commits: usize,
    /// 与L1的差异commit数
    pub diff_commits: usize,
}

impl L0PollingResult {
    pub fn new() -> Self {
        Self {
            polled_at: Utc::now(),
            new_commits: 0,
            diff_commits: 0,
        }
    }
