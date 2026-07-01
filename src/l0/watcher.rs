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

use std::collections::HashSet;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::json;

use crate::collectors::traits::{CollectConfig, Collector, Platform};
use crate::entities::{l0_commits, packages};
use crate::telemetry::Telemetry;

/// L0 仓库拉取摘要
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct L0PollSummary {
    pub commits_inserted: usize,
    pub commits_skipped: usize,
}

/// L0 仓库监听器
pub struct L0Watcher<'a, C>
where
    C: Collector + Send + Sync + ?Sized,
{
    db: &'a DatabaseConnection,
    collector: &'a C,
