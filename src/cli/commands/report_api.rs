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

//! 报告查询命令实现（基于 API）
//!
//! 通过 HTTP API 查询和导出报告

use anyhow::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;

use crate::cli::client::ApiClient;
use crate::cli::formatter::format_datetime_local;

/// 报告摘要
#[derive(Debug, Serialize, Deserialize)]
struct ReportSummary {
    id: i64,
    tracking_id: i32,
    report_type: String,
    package_name: String,
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

/// 报告详情
#[derive(Debug, Serialize, Deserialize)]
struct ReportDetail {
    id: i64,
    tracking_id: i32,
    report_type: String,
    package_name: String,
    status: String,
    content: serde_json::Value,
