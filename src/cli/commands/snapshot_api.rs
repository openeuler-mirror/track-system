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

