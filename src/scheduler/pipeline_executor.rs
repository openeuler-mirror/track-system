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

//! 流水线执行器
//!
//! 负责执行完整的同步流水线：L1获取 → L2快照 → 差异对比 → 分类 → 报告 → 回合建议

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::entities::{sync_jobs, tracking};
use crate::telemetry::Telemetry;

use super::{SyncApiClient, SyncManager};

/// 流水线阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PipelineStage {
    L1Ingestion,
    L2Snapshot,
    DiffComparison,
    Classification,
    ReportGeneration,
    BackportSuggestion,
}

impl PipelineStage {
    /// 获取阶段名称
    pub fn name(&self) -> &'static str {
        match self {
            Self::L1Ingestion => "L1 元数据获取",
            Self::L2Snapshot => "L2 快照生成",
            Self::DiffComparison => "差异对比",
            Self::Classification => "变更分类",
            Self::ReportGeneration => "报告生成",
