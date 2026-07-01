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
