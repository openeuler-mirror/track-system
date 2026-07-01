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

use crate::telemetry::Telemetry;
/// 元数据导出器
///
/// 功能：
/// - 支持 JSON 和 SQL 格式导出
/// - 支持增量和全量导出
/// - 包含文件完整性校验
use chrono::{DateTime, Utc};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 导出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    /// JSON 格式
    Json,
    /// SQL 格式
    Sql,
}

/// 导出选项
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// 导出格式
    pub format: ExportFormat,
    /// 是否包含 commit 记录
    pub include_commits: bool,
    /// 是否增量导出
    pub incremental: bool,
    /// 增量导出起始时间
    pub since: Option<DateTime<Utc>>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Json,
