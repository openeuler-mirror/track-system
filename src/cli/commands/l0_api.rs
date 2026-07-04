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
