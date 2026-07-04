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
