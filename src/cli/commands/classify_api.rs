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

//! Classify 命令处理器（通过 API）

use anyhow::Result;
use colored::Colorize;

use crate::cli::{client::ApiClient, parser::ClassifyAction};

/// 执行分类命令
pub async fn execute(api_client: &ApiClient, action: ClassifyAction) -> Result<()> {
    match action {
        ClassifyAction::Process { limit } => process_classification(api_client, limit).await,
        ClassifyAction::ProcessTracking { tracking_id, limit } => {
            process_tracking_classification(api_client, tracking_id, limit).await
        }
        ClassifyAction::Daemon {
            interval,
            batch_size,
        } => run_classification_daemon(api_client, interval, batch_size).await,
    }
}

/// 处理待分类的 commits
async fn process_classification(api_client: &ApiClient, limit: usize) -> Result<()> {
    println!(
        "{}",
        format!("正在处理待分类的 commits (限制: {})...", limit).cyan()
    );

    let result: serde_json::Value = api_client
