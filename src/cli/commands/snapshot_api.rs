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

//! Snapshot 命令处理器（通过 API）

use anyhow::Result;
use colored::Colorize;

use crate::cli::formatter::format_datetime_local;
use crate::cli::{client::ApiClient, parser::SnapshotAction};

/// 执行快照命令
pub async fn execute(api_client: &ApiClient, action: SnapshotAction) -> Result<()> {
    match action {
        SnapshotAction::Create { tracking_id, tag } => {
            create_snapshot(api_client, tracking_id, tag).await
        }
        SnapshotAction::Restore { snapshot_id, force } => {
            restore_snapshot(api_client, snapshot_id, force).await
        }
        SnapshotAction::List { tracking_id } => list_snapshots(api_client, tracking_id).await,
        SnapshotAction::Delete { snapshot_id } => delete_snapshot(api_client, snapshot_id).await,
    }
}

/// 创建 L2 快照
async fn create_snapshot(
    api_client: &ApiClient,
    tracking_id: i32,
    tag: Option<String>,
) -> Result<()> {
    println!(
        "{}",
        format!("正在创建 tracking {} 的快照...", tracking_id).cyan()
    );

    let mut payload = serde_json::json!({
        "tracking_id": tracking_id
    });

    if let Some(t) = tag {
        payload["tag"] = serde_json::Value::String(t);
    }

    let result: serde_json::Value = api_client.post("/snapshot/create", &payload).await?;

    println!("{}", "✓ 快照已创建".green());
    println!("快照 ID: {}", result["snapshot_id"]);
    let created_at = result["created_at"].as_str().unwrap_or("-");
    let created_at = chrono::DateTime::parse_from_rfc3339(created_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .ok()
        .map(|dt| format_datetime_local(&dt))
        .unwrap_or_else(|| created_at.to_string());
    println!("创建时间: {}", created_at);
    if let Some(tag_value) = result["tag"].as_str() {
        println!("标签: {}", tag_value);
    }

    Ok(())
}

/// 恢复 L2 快照
async fn restore_snapshot(api_client: &ApiClient, snapshot_id: i64, force: bool) -> Result<()> {
    println!("{}", format!("正在恢复快照 {}...", snapshot_id).cyan());

    if !force {
        println!("{}", "警告: 此操作将覆盖现有数据".yellow());
        print!("是否继续? (y/N): ");
        use std::io::{self, Write};
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("已取消");
            return Ok(());
        }
    }
