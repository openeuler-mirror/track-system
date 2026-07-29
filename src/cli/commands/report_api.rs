/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
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
    maintenance_summary: Option<ReportMaintenanceSummary>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReportMaintenanceSummary {
    report_id: i64,
    overall_risk: String,
    confidence: String,
    generated_at: chrono::DateTime<chrono::Utc>,
    commit_total: Option<i64>,
    commits_last_12_months: Option<i64>,
    committers_last_12_months: Option<i64>,
    last_commit_at: Option<String>,
    stars: Option<i64>,
    forks: Option<i64>,
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
pub async fn show_report(api_client: &ApiClient, id: i64, show_all: bool) -> Result<()> {
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

            if let Some(maintenance) = &report.maintenance_summary {
                println!();
                println!("{}", "关联 Maintenance 摘要:".bold());
                println!("  报告 ID: {}", maintenance.report_id);
                println!("  风险等级: {}", maintenance.overall_risk);
                println!("  置信度: {}", maintenance.confidence);
                println!(
                    "  报告时间: {}",
                    format_datetime_local(&maintenance.generated_at)
                );
                println!(
                    "  Commit 总数: {}",
                    maintenance
                        .commit_total
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "  近 12 月 Commit 数: {}",
                    maintenance
                        .commits_last_12_months
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "  近 12 月 Committer 数: {}",
                    maintenance
                        .committers_last_12_months
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "  最近一次 Commit 时间: {}",
                    maintenance
                        .last_commit_at
                        .clone()
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "  Stars/Forks: {}/{}",
                    maintenance
                        .stars
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    maintenance
                        .forks
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
            }

            print_version_lifecycle_details(&report.content);
            print_ai_analysis_details(&report.content);

            println!();
            println!("{}", "报告内容:".bold());
            let content = report_content_for_display(&report.content, show_all);
            println!("{}", serde_json::to_string_pretty(&content)?);

            Ok(())
        }
        Err(e) => {
            println!("{} 获取报告详情失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

fn report_content_for_display(content: &serde_json::Value, show_all: bool) -> serde_json::Value {
    if show_all {
        return content.clone();
    }

    let mut content = content.clone();
    if let Some(object) = content.as_object_mut() {
        object.remove("l1_vs_l0");
        object.remove("version_warnings");
    }
    content
}

fn print_ai_analysis_details(content: &serde_json::Value) {
    let Some(lines) = ai_analysis_lines(content) else {
        return;
    };

    println!();
    println!("{}", "AI 评估:".bold());
    for line in lines {
        println!("  {}", line);
    }
}

fn ai_analysis_lines(content: &serde_json::Value) -> Option<Vec<String>> {
    let ai = content.get("ai_analysis")?;
    let mut lines = Vec::new();

    if ai.is_null() {
        lines.push("状态: 未生成".to_string());
        return Some(lines);
    }

    if let Some(error) = ai.get("error").and_then(text_value_from_value) {
        lines.push("状态: 生成失败".to_string());
        lines.push(format!("错误: {}", error));
        return Some(lines);
    }

    lines.push(format!("模型: {}", display_value(ai.get("model"))));
    lines.push(format!(
        "远端模型: {}",
        display_bool(ai.get("used_remote_model"))
    ));
    lines.push(format!(
        "外部公开信息: {}",
        display_bool(ai.get("external_research_used"))
    ));
    lines.push(format!("风险等级: {}", display_value(ai.get("risk"))));
    lines.push(format!("置信度: {}", display_value(ai.get("confidence"))));

    if let Some(summary) = ai.get("summary").and_then(text_value_from_value) {
        lines.push(format!("摘要: {}", summary));
    }

    let actions = ai
        .get("recommended_actions")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(text_value_from_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !actions.is_empty() {
        lines.push("建议动作:".to_string());
        for action in actions.into_iter().take(5) {
            lines.push(format!("  - {}", action));
        }
    }

    let external_references = ai
        .get("external_references")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(text_value_from_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !external_references.is_empty() {
        lines.push("外部参考:".to_string());
        for reference in external_references.into_iter().take(5) {
            lines.push(format!("  - {}", reference));
        }
    }

    let sources_to_check = ai
        .get("sources_to_check")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(text_value_from_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !sources_to_check.is_empty() {
        lines.push("建议核验来源:".to_string());
        for source in sources_to_check.into_iter().take(5) {
            lines.push(format!("  - {}", source));
        }
    }

    Some(lines)
}

fn print_version_lifecycle_details(content: &serde_json::Value) {
    let Some(lines) = version_lifecycle_lines(content) else {
        return;
    };

    println!();
    println!("{}", "版本生命周期评估:".bold());
    for line in lines {
        println!("  {}", line);
    }
}

fn version_lifecycle_lines(content: &serde_json::Value) -> Option<Vec<String>> {
    let l1_vs_l0 = content.get("l1_vs_l0")?;
    let mut lines = Vec::new();

    if l1_vs_l0.is_null() {
        lines.push("L1 vs L0 子报告: 未生成".to_string());
        lines.push("停维生命周期识别: -".to_string());
        lines.push("LTS 判断: -".to_string());
        lines.push("过时版本评估: -".to_string());
        append_version_warnings(&mut lines, content);
        return Some(lines);
    }

    append_maintenance_status(&mut lines, l1_vs_l0);
    append_lts_assessment(&mut lines, l1_vs_l0);
    append_outdated_version(&mut lines, l1_vs_l0);
    append_version_warnings(&mut lines, content);

    Some(lines)
}

fn append_maintenance_status(lines: &mut Vec<String>, l1_vs_l0: &serde_json::Value) {
    let Some(status) = l1_vs_l0.get("maintenance_status") else {
        lines.push("停维生命周期识别: -".to_string());
        return;
    };

    lines.push(format!(
        "停维生命周期识别: {}",
        display_value(status.get("status"))
    ));
    lines.push(format!(
        "停维信息命中: {}",
        display_bool(status.get("stop_maintenance_detected"))
    ));
    lines.push(format!(
        "停维识别置信度: {}",
        display_value(status.get("confidence"))
    ));

    if let Some(notice) = status
        .get("matched_notice")
        .filter(|value| value.is_object())
    {
        lines.push(format!(
            "命中公告版本: {}",
            display_value(notice.get("version"))
        ));
        lines.push(format!(
            "命中公告系列: {}",
            display_value(notice.get("series"))
        ));
        lines.push(format!(
            "维护截止日期: {}",
            display_value(notice.get("support_until"))
        ));
        lines.push(format!("公告来源: {}", display_value(notice.get("source"))));
        if let Some(evidence) = text_value(notice.get("evidence")) {
            lines.push(format!("公告片段: {}", evidence));
        }
    }

    if let Some(evidence) = status.get("evidence").and_then(short_evidence_text) {
        lines.push(format!("停维候选证据: {}", evidence));
    }
}

fn append_lts_assessment(lines: &mut Vec<String>, l1_vs_l0: &serde_json::Value) {
    let Some(lts) = l1_vs_l0.get("lts") else {
        lines.push("LTS 判断: -".to_string());
        return;
    };

    lines.push(format!("LTS 判断: {}", display_lts_bool(lts.get("is_lts"))));
    lines.push(format!("LTS 来源: {}", display_value(lts.get("source"))));

    if let Some(evidence) = lts.get("evidence").and_then(short_evidence_text) {
        lines.push(format!("LTS 证据: {}", evidence));
    }
}

fn append_outdated_version(lines: &mut Vec<String>, l1_vs_l0: &serde_json::Value) {
    let Some(outdated) = l1_vs_l0.get("outdated_version") else {
        lines.push("过时版本评估: -".to_string());
        return;
    };

    lines.push(format!(
        "过时版本评估: {}",
        display_bool(outdated.get("is_outdated"))
    ));
    lines.push(format!(
        "当前组件版本: {}",
        display_value(outdated.get("current_version"))
    ));
    lines.push(format!(
        "最新版本(L1): {}",
        display_value(outdated.get("latest_version"))
    ));
    lines.push(format!(
        "最新版本来源: {}",
        display_value(outdated.get("latest_version_source"))
    ));
    lines.push(format!(
        "主线版本(L0): {}",
        display_value(outdated.get("mainline_version"))
    ));
    lines.push(format!(
        "主线版本来源: {}",
        display_value(outdated.get("mainline_version_source"))
    ));
    lines.push(format!(
        "大版本差距: {} / {}",
        display_value(outdated.get("major_version_gap")),
        display_value(outdated.get("threshold_major_versions"))
    ));
}

fn append_version_warnings(lines: &mut Vec<String>, content: &serde_json::Value) {
    let warnings = content
        .get("version_warnings")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(text_value_from_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if warnings.is_empty() {
        lines.push("版本风险提示: 无".to_string());
    } else {
        lines.push("版本风险提示:".to_string());
        for warning in warnings {
            lines.push(format!("  - {}", warning));
        }
    }
}

fn display_lts_bool(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::Bool(true)) => "是".to_string(),
        Some(serde_json::Value::Bool(false)) => "否".to_string(),
        Some(serde_json::Value::String(text)) if text.eq_ignore_ascii_case("true") => {
            "是".to_string()
        }
        Some(serde_json::Value::String(text)) if text.eq_ignore_ascii_case("false") => {
            "否".to_string()
        }
        _ => "未知".to_string(),
    }
}

fn display_bool(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::Bool(true)) => "是".to_string(),
        Some(serde_json::Value::Bool(false)) => "否".to_string(),
        Some(serde_json::Value::String(text)) if text.eq_ignore_ascii_case("true") => {
            "是".to_string()
        }
        Some(serde_json::Value::String(text)) if text.eq_ignore_ascii_case("false") => {
            "否".to_string()
        }
        _ => "-".to_string(),
    }
}

fn display_value(value: Option<&serde_json::Value>) -> String {
    value
        .and_then(text_value_from_value)
        .unwrap_or_else(|| "-".to_string())
}

fn text_value(value: Option<&serde_json::Value>) -> Option<String> {
    value.and_then(text_value_from_value)
}

fn text_value_from_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        serde_json::Value::Bool(flag) => Some(flag.to_string()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Array(items) => {
            let values = items
                .iter()
                .filter_map(text_value_from_value)
                .collect::<Vec<_>>();
            if values.is_empty() {
                None
            } else {
                Some(values.join("；"))
            }
        }
        serde_json::Value::Object(_) => serde_json::to_string(value).ok(),
    }
}

fn short_evidence_text(value: &serde_json::Value) -> Option<String> {
    let items = value.as_array()?;
    let evidence = items
        .iter()
        .take(3)
        .filter_map(|item| {
            item.get("evidence")
                .and_then(text_value_from_value)
                .or_else(|| text_value_from_value(item))
        })
        .collect::<Vec<_>>();

    if evidence.is_empty() {
        None
    } else {
        Some(evidence.join("；"))
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
        .await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_reports_empty() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/reports?page=1&page_size=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "data": {
                        "items": [],
                        "total": 0,
                        "page": 1,
                        "page_size": 10,
                        "total_pages": 0
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
    async fn test_show_report() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/reports/123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "data": {
                        "id": 123,
                        "tracking_id": 10,
                        "report_type": "comparison",
                        "package_name": "test-package",
                        "status": "completed",
                        "content": {
                            "summary": "Test report content",
                            "changes": 10
                        },
                        "created_at": "2024-01-01T00:00:00Z",
                        "updated_at": "2024-01-01T01:00:00Z"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = show_report(&client, 123).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_export_report_to_console() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/reports/456/export?format=json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("\"{\\\"exported\\\": \\\"data\\\"}\"")
            .create_async()
            .await;

        let result = export_report(&client, 456, "json".to_string(), None).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_export_report_to_file() {
        let (mut server, client) = setup_test_server().await;
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_str().unwrap().to_string();

        let mock = server
            .mock("GET", "/api/reports/789/export?format=csv")
            .with_status(200)
            .with_header("content-type", "text/csv")
            .with_body("\"col1,col2\\nval1,val2\"")
            .create_async()
            .await;

        let result = export_report(&client, 789, "csv".to_string(), Some(file_path.clone())).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        assert!(std::path::Path::new(&file_path).exists());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_show_report_with_different_status() {
        let (mut server, client) = setup_test_server().await;

        // Test pending status
        let mock = server
            .mock("GET", "/api/reports/111")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "data": {
                        "id": 111,
                        "tracking_id": 1,
                        "report_type": "analysis",
                        "package_name": "pkg",
                        "status": "pending",
                        "content": {},
                        "created_at": "2024-01-01T00:00:00Z",
                        "updated_at": "2024-01-01T00:00:00Z"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = show_report(&client, 111).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }
}
