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

//! 系统状态查询命令实现（基于 API）
//!
//! 通过 HTTP API 查询系统状态

use anyhow::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::cli::client::ApiClient;
use crate::cli::parser::StatusAction;

/// 系统状态响应
#[derive(Debug, Serialize, Deserialize)]
struct SystemStatus {
    status: String,
    version: String,
    uptime: u64,
    database: DatabaseStatus,
    scheduler: SchedulerStatus,
}

/// 数据库状态
#[derive(Debug, Serialize, Deserialize)]
struct DatabaseStatus {
    connected: bool,
    pool_size: usize,
}

/// 调度器状态
#[derive(Debug, Serialize, Deserialize)]
struct SchedulerStatus {
    running: bool,
    active_jobs: usize,
    pending_jobs: usize,
}

/// API 响应包装
#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T> {
    data: T,
}

/// 执行状态查询命令
pub async fn execute(api_client: &ApiClient, action: StatusAction) -> Result<()> {
    match action {
        StatusAction::Overview => show_overview(api_client).await,
        StatusAction::Scheduler => show_scheduler(api_client).await,
        StatusAction::RateLimit => show_rate_limit(api_client).await,
    }
}

/// 显示系统概览
async fn show_overview(api_client: &ApiClient) -> Result<()> {
    println!("正在获取系统状态...");
    println!();

    match api_client.get::<ApiResponse<SystemStatus>>("/status").await {
        Ok(response) => {
            let status = response.data;

            println!("{}", "系统状态概览:".bold());
            println!();

            // 系统状态
            let status_str = match status.status.as_str() {
                "healthy" => "健康".green(),
                "degraded" => "降级".yellow(),
                "unhealthy" => "异常".red(),
                _ => status.status.as_str().into(),
            };
            println!("  状态: {}", status_str);
            println!("  版本: {}", status.version.cyan());
            println!("  运行时间: {} 秒", status.uptime);
            println!();

            // 数据库状态
            println!("{}", "数据库:".bold());
            println!(
                "  连接状态: {}",
                if status.database.connected {
                    "已连接".green()
                } else {
                    "未连接".red()
                }
            );
            println!("  连接池大小: {}", status.database.pool_size);
            println!();

            // 调度器状态
            println!("{}", "调度器:".bold());
            println!(
                "  运行状态: {}",
                if status.scheduler.running {
                    "运行中".green()
                } else {
                    "已停止".red()
                }
            );
            println!("  活动任务: {}", status.scheduler.active_jobs);
            println!("  待处理任务: {}", status.scheduler.pending_jobs);

            Ok(())
        }
        Err(e) => {
            println!("{} 获取系统状态失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 显示调度器状态
async fn show_scheduler(api_client: &ApiClient) -> Result<()> {
    println!("正在获取调度器状态...");
    println!();

    match api_client
        .get::<ApiResponse<SchedulerStatus>>("/status/scheduler")
        .await
    {
        Ok(response) => {
            let scheduler = response.data;

            println!("{}", "调度器状态:".bold());
            println!(
                "  运行状态: {}",
                if scheduler.running {
                    "运行中".green()
                } else {
                    "已停止".red()
                }
            );
            println!("  活动任务: {}", scheduler.active_jobs);
            println!("  待处理任务: {}", scheduler.pending_jobs);

            Ok(())
        }
        Err(e) => {
            println!("{} 获取调度器状态失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 显示速率限制状态
async fn show_rate_limit(api_client: &ApiClient) -> Result<()> {
    println!("正在获取速率限制状态...");
    println!();

    match api_client
        .get::<serde_json::Value>("/status/rate-limit")
        .await
    {
        Ok(response) => {
            println!("{}", "速率限制状态:".bold());
            println!("{}", serde_json::to_string_pretty(&response)?);
            Ok(())
        }
        Err(e) => {
            println!("{} 获取速率限制状态失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

