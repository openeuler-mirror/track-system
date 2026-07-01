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

//! Sync 命令处理器（通过 API）

use anyhow::Result;
use colored::Colorize;

use crate::cli::{client::ApiClient, parser::SyncAction};

/// 执行同步命令
pub async fn execute(api_client: &ApiClient, action: SyncAction) -> Result<()> {
    match action {
        SyncAction::Run {
            tracking_id,
            timeout: _,
            continue_on_error: _,
        } => run_sync(api_client, tracking_id).await,
        SyncAction::RunAll { concurrency: _ } => run_all_sync(api_client).await,
        SyncAction::Batch {
            ids,
            concurrency: _,
        } => batch_sync(api_client, ids).await,
        SyncAction::Wake { tracking_id } => wake_scheduler(api_client, tracking_id).await,
        SyncAction::Status => show_sync_status(api_client).await,
    }
}

/// 执行单个 tracking 的同步
async fn run_sync(api_client: &ApiClient, tracking_id: i32) -> Result<()> {
    println!("{}", "正在提交同步任务...".cyan());

    let result: serde_json::Value = api_client
        .post(
            &format!("/sync/{}/queue", tracking_id),
            &serde_json::json!({}),
        )
        .await?;

    println!("{}", "✓ 同步任务已提交".green());
    println!("任务 ID: {}", result["job_id"]);
    println!("状态: {}", result["status"]);

    Ok(())
}

/// 执行所有待处理的同步任务
async fn run_all_sync(api_client: &ApiClient) -> Result<()> {
    println!("{}", "正在获取所有待同步的 tracking...".cyan());

    // 获取所有 active 状态的 tracking
    let result: serde_json::Value = api_client.get("/tracking?status=active").await?;
    let trackings = result["data"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("无效的响应格式"))?;

    if trackings.is_empty() {
        println!("{}", "没有待同步的 tracking".yellow());
        return Ok(());
    }

    println!("找到 {} 个待同步的 tracking", trackings.len());

    let mut success_count = 0;
    let mut failed_count = 0;

    for tracking in trackings {
        let tracking_id = tracking["id"]
