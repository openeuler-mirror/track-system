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
            include_commits: false,
            incremental: false,
            since: None,
        }
    }
}

/// 导出结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    /// 是否成功
    pub success: bool,
    /// 导出的软件包数量
    pub exported_packages: usize,
    /// 导出的发行版数量
    pub exported_distros: usize,
    /// 导出的跟踪配置数量
    pub exported_trackings: usize,
    /// 导出的 commit 数量
    pub exported_commits: usize,
    /// 导出时间
    pub export_time: DateTime<Utc>,
    /// 文件校验和
    pub checksum: Option<String>,
    /// 错误信息
    pub error: Option<String>,
}

impl Default for ExportResult {
    fn default() -> Self {
        Self {
            success: false,
            exported_packages: 0,
            exported_distros: 0,
            exported_trackings: 0,
            exported_commits: 0,
            export_time: Utc::now(),
            checksum: None,
            error: None,
        }
    }
}

/// 导出的元数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedMetadata {
    /// 导出时间
    pub export_time: DateTime<Utc>,
    /// 软件包列表
    pub packages: Vec<serde_json::Value>,
