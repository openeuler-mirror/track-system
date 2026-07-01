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
        .get::<ApiResponse<PaginatedResponse<ReportSummary>>>(&format!("/reports{}", query))
        .await
    {
        Ok(response) => {
            let data = response.data;

            if data.items.is_empty() {
                println!("{}", "没有找到报告".yellow());
                return Ok(());
            }

            println!();
            println!("{}", "报告列表:".bold());
            println!(
                "{:<8} {:<15} {:<20} {:<30} {:<12} {:<20}",
                "ID", "跟踪ID", "类型", "软件包", "状态", "创建时间"
            );
            println!("{}", "-".repeat(105));

            for report in data.items {
                let status_str = match report.status.as_str() {
                    "completed" => "已完成".green(),
                    "pending" => "等待中".yellow(),
                    "failed" => "失败".red(),
                    _ => report.status.as_str().into(),
                };

                println!(
                    "{:<8} {:<15} {:<20} {:<30} {} {:<20}",
                    report.id,
                    report.tracking_id,
                    report.report_type,
                    report.package_name,
                    status_str,
                    format_datetime_local(&report.created_at)
                );
            }

            println!();
            println!(
                "第 {}/{} 页，共 {} 条记录",
                data.page, data.total_pages, data.total
            );

            Ok(())
        }
        Err(e) => {
            println!("{} 获取报告列表失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 显示报告详情
pub async fn show_report(api_client: &ApiClient, id: i64) -> Result<()> {
    println!("正在获取报告详情...");
    println!("  报告 ID: {}", id);
    println!();

    match api_client
        .get::<ApiResponse<ReportDetail>>(&format!("/reports/{}", id))
        .await
    {
        Ok(response) => {
            let report = response.data;

            println!("{}", "报告详情:".bold());
            println!("  ID: {}", report.id);
            println!("  跟踪配置 ID: {}", report.tracking_id);
            println!("  报告类型: {}", report.report_type.cyan());
            println!("  软件包: {}", report.package_name.cyan());

            let status_str = match report.status.as_str() {
                "completed" => "已完成".green(),
                "pending" => "等待中".yellow(),
                "failed" => "失败".red(),
                _ => report.status.as_str().into(),
            };
            println!("  状态: {}", status_str);

            println!("  创建时间: {}", format_datetime_local(&report.created_at));
            println!("  更新时间: {}", format_datetime_local(&report.updated_at));

            println!();
            println!("{}", "报告内容:".bold());
            println!("{}", serde_json::to_string_pretty(&report.content)?);

            Ok(())
        }
        Err(e) => {
            println!("{} 获取报告详情失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 导出报告
pub async fn export_report(
    api_client: &ApiClient,
    id: i64,
    format: String,
    output: Option<String>,
) -> Result<()> {
    println!("正在导出报告...");
    println!("  报告 ID: {}", id);
    println!("  格式: {}", format);

    match api_client
        .get::<String>(&format!("/reports/{}/export?format={}", id, format))
        .await
    {
        Ok(content) => {
            if let Some(output_path) = output {
                // 保存到文件
                fs::write(&output_path, &content)?;
                println!();
                println!("{} 报告已导出", "✓".green().bold());
                println!("  文件: {}", output_path.cyan());
            } else {
                // 输出到控制台
                println!();
                println!("{}", "报告内容:".bold());
                println!("{}", content);
            }

            Ok(())
        }
        Err(e) => {
            println!("{} 导出报告失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::client::ClientConfig;
    use mockito::Server;
    use tempfile::NamedTempFile;

    async fn setup_test_server() -> (mockito::ServerGuard, ApiClient) {
        let server = Server::new_async().await;
        let config = ClientConfig {
            server_url: server.url(),
            auth_token: Some("test_token".to_string()),
            timeout: 30,
            verify_ssl: true,
        };
        let client = ApiClient::new(config).unwrap();
        (server, client)
    }

    #[tokio::test]
    async fn test_list_reports() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/reports?page=1&page_size=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "data": {
                        "items": [
                            {
                                "id": 1,
                                "tracking_id": 10,
                                "report_type": "comparison",
                                "package_name": "test-package",
                                "status": "completed",
                                "created_at": "2024-01-01T00:00:00Z",
                                "updated_at": "2024-01-01T01:00:00Z"
                            }
                        ],
                        "total": 1,
                        "page": 1,
                        "page_size": 10,
                        "total_pages": 1
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = list_reports(&client, None, None, None, None).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_reports_with_filters() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock(
                "GET",
                "/api/reports?page=2&page_size=20&tracking_id=5&report_type=diff",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "data": {
                        "items": [],
                        "total": 0,
                        "page": 2,
                        "page_size": 20,
                        "total_pages": 0
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = list_reports(
            &client,
            Some(2),
            Some(20),
            Some(5),
            Some("diff".to_string()),
        )
