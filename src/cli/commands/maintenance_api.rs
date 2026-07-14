use anyhow::{anyhow, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::client::ApiClient;
use crate::cli::dto::{MaintenanceRefreshResultDto, MaintenanceReportDto, PackageDto};
use crate::cli::formatter::format_datetime_local;
use crate::cli::parser::MaintenanceAction;

#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T> {
    data: Option<T>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PaginatedResponse<T> {
    items: Vec<T>,
    total: u64,
    page: u64,
    page_size: u64,
    total_pages: u64,
}

pub async fn execute(api_client: &ApiClient, action: MaintenanceAction) -> Result<()> {
    match action {
        MaintenanceAction::Refresh { package } => {
            let package_id = resolve_package_id(api_client, &package).await?;
            refresh_package(api_client, package_id).await
        }
        MaintenanceAction::LatestReport { package, verbose } => {
            let package_id = resolve_package_id(api_client, &package).await?;
            show_latest_report(api_client, package_id, verbose).await
        }
        MaintenanceAction::Reports {
            page,
            page_size,
            package,
            report_type,
        } => {
            let package_id = match package {
                Some(package) => Some(resolve_package_id(api_client, &package).await?),
                None => None,
            };
            list_reports(api_client, page, page_size, package_id, report_type).await
        }
        MaintenanceAction::Report { id, verbose } => show_report(api_client, id, verbose).await,
    }
}

async fn resolve_package_id(api_client: &ApiClient, input: &str) -> Result<i32> {
    if let Ok(id) = input.parse::<i32>() {
        return Ok(id);
    }

    let packages = api_client.get::<Vec<PackageDto>>("/packages").await?;
    packages
        .into_iter()
        .find(|package| package.name == input)
        .map(|package| package.id)
        .ok_or_else(|| anyhow!("未找到软件包: {}", input))
}

async fn refresh_package(api_client: &ApiClient, package_id: i32) -> Result<()> {
    println!(
        "正在刷新维护评估: package={}",
        package_id.to_string().cyan()
    );
    let response = api_client
        .post::<_, ApiResponse<MaintenanceRefreshResultDto>>(
            &format!("/maintenance/packages/{}/refresh", package_id),
            &serde_json::json!({}),
        )
        .await?;
    let result = response
        .data
        .ok_or_else(|| anyhow!("服务端未返回刷新结果"))?;
    println!(
        "{} 刷新完成: package_id={}, evidence_count={}, report_id={}, generated_at={}",
        "✓".green().bold(),
        result.package_id,
        result.evidence_count,
        result.report_id,
        format_datetime_local(&result.generated_at)
    );
    Ok(())
}

async fn show_latest_report(api_client: &ApiClient, package_id: i32, verbose: bool) -> Result<()> {
    let response = api_client
        .get::<ApiResponse<MaintenanceReportDto>>(&format!(
            "/maintenance/packages/{}/latest-report",
            package_id
        ))
        .await?;
    let report = response
        .data
        .ok_or_else(|| anyhow!("服务端未返回维护评估报告"))?;
    print_report_detail(&report, verbose);
    Ok(())
}

async fn list_reports(
    api_client: &ApiClient,
    page: u64,
    page_size: u64,
    package_id: Option<i32>,
    report_type: Option<String>,
) -> Result<()> {
    let mut query = format!("?page={}&page_size={}", page, page_size);
    if let Some(package_id) = package_id {
        query.push_str(&format!("&package_id={}", package_id));
    }
    if let Some(report_type) = report_type {
        query.push_str(&format!("&report_type={}", report_type));
    }

    let response = api_client
        .get::<ApiResponse<PaginatedResponse<MaintenanceReportDto>>>(&format!(
            "/maintenance/reports{}",
            query
        ))
        .await?;
    let data = response
        .data
        .ok_or_else(|| anyhow!("服务端未返回维护评估报告列表"))?;

    if data.items.is_empty() {
        println!("{}", "没有找到维护评估报告".yellow());
        return Ok(());
    }

    println!("{}", "维护评估报告列表:".bold());
    for item in data.items {
        println!(
            "  [{}] package={} risk={} confidence={} generated_at={}",
            item.id,
            item.package_id,
            item.overall_risk,
            item.confidence,
            format_datetime_local(&item.generated_at)
        );
    }
    Ok(())
}

async fn show_report(api_client: &ApiClient, id: i64, verbose: bool) -> Result<()> {
    let response = api_client
        .get::<ApiResponse<MaintenanceReportDto>>(&format!("/maintenance/reports/{}", id))
        .await?;
    let report = response
        .data
        .ok_or_else(|| anyhow!("服务端未返回维护评估报告"))?;
    print_report_detail(&report, verbose);
    Ok(())
}

fn print_report_detail(report: &MaintenanceReportDto, verbose: bool) {
    println!();
    println!("{}", "维护评估报告详情:".bold());
    println!("  ID: {}", report.id);
    println!("  软件包 ID: {}", report.package_id);
    println!("  报告类型: {}", report.report_type);
    println!("  状态: {}", report.status);
    println!("  综合风险: {}", report.overall_risk);
    println!("  置信度: {}", report.confidence);
    println!("  摘要: {}", report.summary);
    print_maintenance_focus_details(&report.report_payload);
    println!("  维度摘要: {}", report.dimensions);
    if verbose {
        if let Some(evidence_summary) = &report.evidence_summary {
            println!("  证据摘要: {}", evidence_summary);
        }
        println!("  报告载荷: {}", report.report_payload);
    } else if let Some(evidence_summary) = &report.evidence_summary {
        println!("  证据摘要: {}", evidence_summary);
    }
    println!(
        "  生成时间: {}",
        format_datetime_local(&report.generated_at)
    );
}

fn print_maintenance_focus_details(report_payload: &Value) {
    let commit_total = find_focus_value(report_payload, "commit_total");
    let commits_last_12_months = find_focus_value(report_payload, "commits_last_12_months");
    let committers_last_12_months = find_focus_value(report_payload, "committers_last_12_months");
    let last_commit_at = find_focus_value(report_payload, "last_commit_at");
    let stars = find_focus_value(report_payload, "stars");
    let forks = find_focus_value(report_payload, "forks");

    if commit_total.is_none()
        && commits_last_12_months.is_none()
        && committers_last_12_months.is_none()
        && last_commit_at.is_none()
        && stars.is_none()
        && forks.is_none()
    {
        return;
    }

    println!("  维护指标:");
    println!(
        "    - Commit 总数: {}",
        commit_total.unwrap_or_else(|| "-".to_string())
    );
    println!(
        "    - 近 12 月 Commit 数: {}",
        commits_last_12_months.unwrap_or_else(|| "-".to_string())
    );
    println!(
        "    - 近 12 月 Committer 数: {}",
        committers_last_12_months.unwrap_or_else(|| "-".to_string())
    );
    println!(
        "    - 最近一次 Commit 时间: {}",
        last_commit_at.unwrap_or_else(|| "-".to_string())
    );
    println!("    - Stars: {}", stars.unwrap_or_else(|| "-".to_string()));
    println!("    - Forks: {}", forks.unwrap_or_else(|| "-".to_string()));
}

fn find_focus_value(report_payload: &Value, key: &str) -> Option<String> {
    find_indicator_json_value(report_payload, key)
        .or_else(|| find_raw_evidence_json_value(report_payload, key))
        .and_then(|value| value_to_readable_text(&value))
}

fn find_indicator_json_value(report_payload: &Value, key: &str) -> Option<Value> {
    report_payload
        .get("section")
        .and_then(|section| section.get("indicators"))
        .and_then(Value::as_array)
        .and_then(|indicators| {
            indicators.iter().find_map(|indicator| {
                let indicator_key = indicator.get("key").and_then(Value::as_str)?;
                if indicator_key == key {
                    indicator.get("value").cloned()
                } else {
                    None
                }
            })
        })
}

fn find_raw_evidence_json_value(report_payload: &Value, key: &str) -> Option<Value> {
    report_payload
        .get("raw_evidence")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries.iter().find_map(|entry| {
                let category = entry.get("assessment_category").and_then(Value::as_str)?;
                if category != "maintenance" {
                    return None;
                }
                entry.get("data").and_then(|data| data.get(key)).cloned()
            })
        })
}

fn value_to_readable_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Array(items) => {
            let values = items
                .iter()
                .filter_map(value_to_readable_text)
                .collect::<Vec<_>>();
            if values.is_empty() {
                None
            } else {
                Some(values.join("；"))
            }
        }
        Value::Object(_) => serde_json::to_string(value).ok(),
    }
}
