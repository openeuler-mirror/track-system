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

//! Distro 命令处理器（通过 API）

use anyhow::Result;
use colored::Colorize;

use crate::cli::{client::ApiClient, parser::DistroAction};

/// 执行发行版管理命令
pub async fn execute(api_client: &ApiClient, action: DistroAction) -> Result<()> {
    match action {
        DistroAction::Add {
            name,
            version,
            description,
        } => add_distro(api_client, name, version, description).await,
        DistroAction::List => list_distros(api_client).await,
        DistroAction::Show { name_or_id } => show_distro(api_client, name_or_id).await,
        DistroAction::Remove { name, confirm } => remove_distro(api_client, name, confirm).await,
    }
}

/// 添加发行版
async fn add_distro(
    api_client: &ApiClient,
    name: String,
    version: String,
    description: Option<String>,
) -> Result<()> {
    println!(
        "{}",
        format!("正在添加发行版 {}:{}...", name, version).cyan()
    );

    let mut payload = serde_json::json!({
        "name": name,
        "version": version
    });

    if let Some(desc) = description {
        payload["description"] = serde_json::Value::String(desc);
    }

    let result: serde_json::Value = api_client.post("/distros", &payload).await?;

    println!("{}", "✓ 发行版已添加".green());
    println!("ID: {}", result["id"]);
    println!("名称: {}", result["name"]);
    println!("版本: {}", result["version"]);

    Ok(())
}

/// 列出所有发行版
async fn list_distros(api_client: &ApiClient) -> Result<()> {
    println!("{}", "正在获取发行版列表...".cyan());

    let result: serde_json::Value = api_client.get("/distros").await?;
    let distros = result["data"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("无效的响应格式"))?;

    if distros.is_empty() {
        println!("{}", "没有发行版".yellow());
        return Ok(());
    }

    println!("\n{}", "=== 发行版列表 ===".bold());
