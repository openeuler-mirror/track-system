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

//! Health 命令处理器（通过 API）

use anyhow::Result;
use colored::Colorize;

use crate::cli::{client::ApiClient, parser::HealthAction};

/// 执行健康检查命令
pub async fn execute(api_client: &ApiClient, action: HealthAction) -> Result<()> {
    match action {
        HealthAction::Check { component } => check_health(api_client, component).await,
    }
}

/// 检查系统健康状态
async fn check_health(api_client: &ApiClient, component: Option<String>) -> Result<()> {
    if let Some(comp) = component {
        check_component_health(api_client, &comp).await
    } else {
        check_all_health(api_client).await
    }
}

/// 检查所有组件健康状态
async fn check_all_health(api_client: &ApiClient) -> Result<()> {
    println!("{}", "正在检查系统健康状态...".cyan());

    let health: serde_json::Value = api_client.get("/health").await?;
