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

//! L1/L2仓库对比服务
//!
//! 负责对比L1（上游）和L2（本地）仓库的差异

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};
use std::path::Path;
use tokio::task;
use tracing::{debug, info};

use super::git_client::GitRepositoryClient;
use crate::entities::{prelude::TrackingReports, tracking, tracking_reports};

/// 差异对比摘要
#[derive(Debug, Clone)]
pub struct ComparisonReport {
    /// 追踪配置ID
    pub tracking_id: i32,
    /// L1落后的commit数
    pub commits_behind: usize,
    /// L1领先的commit数
    pub commits_ahead: usize,
    /// 差异摘要（JSON）
    pub diff_summary: serde_json::Value,
    /// 生成源
    pub source: String,
}

/// L1/L2仓库对比服务
pub struct ComparisonService<'a> {
    db: &'a DatabaseConnection,
}
