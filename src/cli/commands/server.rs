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

//! 服务器管理命令实现
//!
//! 提供服务器连接配置、测试和信息查询功能

use anyhow::Result;
use colored::Colorize;

use crate::cli::client::{ApiClient, ClientConfig};
use crate::cli::parser::ServerAction;

/// 执行服务器管理命令
pub async fn execute(api_client: &ApiClient, action: ServerAction) -> Result<()> {
    match action {
        ServerAction::Config { url, token, show } => {
            execute_config(api_client, url, token, show).await
        }
        ServerAction::Ping => execute_ping(api_client).await,
        ServerAction::Info => execute_info(api_client).await,
    }
}

/// 配置服务器连接
async fn execute_config(
    _api_client: &ApiClient,
    url: Option<String>,
    token: Option<String>,
