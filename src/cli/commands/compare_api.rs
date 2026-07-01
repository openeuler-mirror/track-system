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

//! 对比分析命令实现（基于 API）
//!
//! 通过 HTTP API 执行对比分析

use anyhow::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::cli::client::ApiClient;
use crate::cli::formatter::format_datetime_local;
use crate::cli::parser::CompareAction;

/// 对比任务状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CompareStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// 对比任务响应
#[derive(Debug, Serialize, Deserialize)]
struct CompareTaskResponse {
    task_id: String,
    status: CompareStatus,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// 对比状态响应
#[derive(Debug, Serialize, Deserialize)]
struct CompareStatusResponse {
    task_id: String,
    status: CompareStatus,
    progress: u8,
    message: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
    report_id: Option<i64>,
}

/// API 响应包装
#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T> {
    data: T,
}

/// L1 vs L0 对比请求
#[derive(Debug, Serialize, Deserialize)]
struct CompareL1VsL0Request {
    tracking_id: i32,
    l0_snapshot_id: Option<String>,
    l1_snapshot_id: Option<String>,
}

/// L2 vs L1 对比请求
#[derive(Debug, Serialize, Deserialize)]
struct CompareL2VsL1Request {
    tracking_id: i32,
    l1_snapshot_id: Option<String>,
    l2_snapshot_id: Option<String>,
}

/// 执行对比命令
pub async fn execute(api_client: &ApiClient, action: CompareAction) -> Result<()> {
    match action {
        CompareAction::Tracking { tracking_id } => {
            // 默认执行 L2 vs L1 对比
            compare_l2_vs_l1(api_client, tracking_id, None, None).await
        }
        CompareAction::Report { format, output } => {
            generate_report(api_client, format, output).await
        }
    }
}

/// 执行 L1 vs L0 对比
pub async fn compare_l1_vs_l0(
    api_client: &ApiClient,
    tracking_id: i32,
    l0_snapshot_id: Option<String>,
    l1_snapshot_id: Option<String>,
) -> Result<()> {
    println!("正在创建 L1 vs L0 对比任务...");
    println!("  跟踪配置 ID: {}", tracking_id);

    let request = CompareL1VsL0Request {
        tracking_id,
        l0_snapshot_id,
        l1_snapshot_id,
    };

    match api_client
        .post::<_, ApiResponse<CompareTaskResponse>>("/compare/l1-vs-l0", &request)
        .await
    {
        Ok(response) => {
            let task = response.data;
            println!();
            println!("{} 对比任务已创建", "✓".green().bold());
            println!("  任务 ID: {}", task.task_id.cyan());
            println!("  状态: {:?}", task.status);
            println!("  创建时间: {}", format_datetime_local(&task.created_at));
            println!();
            println!("使用以下命令查询任务状态:");
            println!("  track-cli compare status {}", task.task_id);
            Ok(())
        }
        Err(e) => {
            println!("{} 创建对比任务失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 执行 L2 vs L1 对比
pub async fn compare_l2_vs_l1(
    api_client: &ApiClient,
    tracking_id: i32,
    l1_snapshot_id: Option<String>,
    l2_snapshot_id: Option<String>,
) -> Result<()> {
    println!("正在创建 L2 vs L1 对比任务...");
    println!("  跟踪配置 ID: {}", tracking_id);

    let request = CompareL2VsL1Request {
        tracking_id,
        l1_snapshot_id,
        l2_snapshot_id,
    };

    match api_client
        .post::<_, ApiResponse<CompareTaskResponse>>("/compare/l2-vs-l1", &request)
        .await
    {
        Ok(response) => {
            let task = response.data;
            println!();
            println!("{} 对比任务已创建", "✓".green().bold());
            println!("  任务 ID: {}", task.task_id.cyan());
            println!("  状态: {:?}", task.status);
            println!("  创建时间: {}", format_datetime_local(&task.created_at));
            println!();
            println!("使用以下命令查询任务状态:");
            println!("  track-cli compare status {}", task.task_id);
            Ok(())
        }
        Err(e) => {
            println!("{} 创建对比任务失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 查询对比任务状态
pub async fn get_compare_status(api_client: &ApiClient, task_id: String) -> Result<()> {
    println!("正在查询对比任务状态...");
    println!("  任务 ID: {}", task_id.cyan());
    println!();

    match api_client
        .get::<ApiResponse<CompareStatusResponse>>(&format!("/compare/tasks/{}", task_id))
        .await
    {
        Ok(response) => {
            let status = response.data;

            println!("{}", "任务状态:".bold());
            println!("  任务 ID: {}", status.task_id.cyan());

            let status_str = match status.status {
                CompareStatus::Pending => "等待中".yellow(),
                CompareStatus::Running => "运行中".blue(),
                CompareStatus::Completed => "已完成".green(),
                CompareStatus::Failed => "失败".red(),
                CompareStatus::Cancelled => "已取消".yellow(),
            };
            println!("  状态: {}", status_str);
            println!("  进度: {}%", status.progress);

            if let Some(msg) = status.message {
                println!("  消息: {}", msg);
            }

            println!("  创建时间: {}", format_datetime_local(&status.created_at));
            println!("  更新时间: {}", format_datetime_local(&status.updated_at));

            if let Some(completed_at) = status.completed_at {
                println!("  完成时间: {}", format_datetime_local(&completed_at));
            }

            if let Some(report_id) = status.report_id {
                println!();
                println!("  报告 ID: {}", report_id.to_string().cyan());
                println!("  查看报告: track-cli report show {}", report_id);
            }

            Ok(())
        }
        Err(e) => {
            println!("{} 查询任务状态失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 取消对比任务
pub async fn cancel_compare_task(api_client: &ApiClient, task_id: String) -> Result<()> {
    println!("正在取消对比任务...");
    println!("  任务 ID: {}", task_id.cyan());

    match api_client
        .delete_no_content(&format!("/compare/tasks/{}", task_id))
        .await
    {
        Ok(_) => {
            println!();
            println!("{} 任务已取消", "✓".green().bold());
            println!("  任务 ID: {}", task_id.cyan());
            Ok(())
        }
        Err(e) => {
            println!("{} 取消任务失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 生成对比报告
async fn generate_report(
    _api_client: &ApiClient,
    format: String,
    output: Option<String>,
) -> Result<()> {
    println!("正在生成对比报告...");
    println!("  格式: {}", format);
    if let Some(path) = output {
        println!("  输出: {}", path);
    }

    // TODO: 实现报告生成逻辑
    println!();
    println!("{}", "注: 此功能待实现".yellow());

    Ok(())
}

