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
