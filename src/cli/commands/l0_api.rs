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

//! L0 命令处理器（通过 API）

use anyhow::Result;
use colored::Colorize;

use crate::cli::{client::ApiClient, parser::L0Action};

/// 执行 L0 命令
pub async fn execute(api_client: &ApiClient, action: L0Action) -> Result<()> {
    match action {
        L0Action::Poll { package_id } => poll_l0(api_client, package_id).await,
        L0Action::DetectDiff { package_id } => detect_diff(api_client, package_id).await,
    }
}

/// 轮询 L0 仓库
async fn poll_l0(api_client: &ApiClient, package_id: Option<i32>) -> Result<()> {
    if let Some(id) = package_id {
        println!("{}", format!("正在轮询 package {}...", id).cyan());

        let result: serde_json::Value = api_client
            .post(&format!("/l0/poll/{}", id), &serde_json::json!({}))
            .await?;

        println!("{}", "✓ 轮询完成".green());
        println!("新 commits: {}", result["new_commits"]);
        println!("新 tags: {}", result["new_tags"]);
        println!("新 releases: {}", result["new_releases"]);
    } else {
        println!("{}", "正在轮询所有 packages...".cyan());

        let result: serde_json::Value = api_client
            .post("/l0/poll/all", &serde_json::json!({}))
            .await?;

        println!("{}", "✓ 轮询完成".green());
        println!("处理的 packages: {}", result["processed"]);
        println!("总新 commits: {}", result["total_new_commits"]);
        println!("总新 tags: {}", result["total_new_tags"]);
        println!("总新 releases: {}", result["total_new_releases"]);
    }

    Ok(())
}

/// 检测 L0 与 L1 的差异
async fn detect_diff(api_client: &ApiClient, package_id: i32) -> Result<()> {
