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
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

/// API 响应包装
#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T> {
    data: T,
}

/// 分页响应
#[derive(Debug, Serialize, Deserialize)]
struct PaginatedResponse<T> {
    items: Vec<T>,
    total: u64,
    page: u64,
    page_size: u64,
    total_pages: u64,
}

/// 列出报告
pub async fn list_reports(
    api_client: &ApiClient,
    page: Option<u64>,
    page_size: Option<u64>,
    tracking_id: Option<i32>,
    report_type: Option<String>,
) -> Result<()> {
    println!("正在获取报告列表...");

    let mut query = format!(
        "?page={}&page_size={}",
        page.unwrap_or(1),
        page_size.unwrap_or(10)
    );

    if let Some(tid) = tracking_id {
        query.push_str(&format!("&tracking_id={}", tid));
    }

    if let Some(rtype) = report_type {
        query.push_str(&format!("&report_type={}", rtype));
    }

    match api_client
