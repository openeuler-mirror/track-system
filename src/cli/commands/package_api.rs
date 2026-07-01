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

//! 软件包管理命令实现（基于 API）
//!
//! 通过 HTTP API 管理软件包

use crate::cli::client::ApiClient;
use crate::cli::dto::{CreatePackageRequest, PackageDto, UpdatePackageRequest};
use crate::cli::formatter::format_datetime_local;
use crate::cli::parser::PackageAction;
use anyhow::{bail, Result};
use colored::Colorize;

fn parse_sync_interval_hours(input: &str) -> Result<i32> {
    let s = input.trim().trim_matches(|c| c == '"' || c == '\'');
    let s = s
        .strip_suffix('h')
        .or_else(|| s.strip_suffix('H'))
        .unwrap_or(s)
        .trim();

    if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit()) {
        bail!("无效的 sync-interval：{input}，格式应为整数小时或以 h 结尾（如 12h）");
    }

    let hours: i32 = s
        .parse()
        .map_err(|_| anyhow::anyhow!("无效的 sync-interval：{input}，无法解析为整数小时"))?;

    let min_hours = 1;
    let max_hours = 24 * 365;
    if !(min_hours..=max_hours).contains(&hours) {
        bail!("无效的 sync-interval：{input}，范围需在 {min_hours}..={max_hours} 小时");
    }

    Ok(hours)
}

/// 辅助：按名称查找软件包（客户端侧过滤）
async fn find_package_by_name(
    api_client: &ApiClient,
    name: &str,
) -> anyhow::Result<Option<PackageDto>> {
    match api_client.get::<Vec<PackageDto>>("/packages").await {
        Ok(list) => Ok(list.into_iter().find(|p| p.name == name)),
        Err(e) => Err(e.into()),
    }
}

/// 执行软件包管理命令
pub async fn execute(api_client: &ApiClient, action: PackageAction) -> Result<()> {
    match action {
        PackageAction::Add {
            name,
            level,
            sync_interval,
            l0_repo,
            description,
        } => add_package(api_client, name, level, sync_interval, l0_repo, description).await,
        PackageAction::List { limit } => list_packages(api_client, limit).await,
        PackageAction::Show { name_or_id } => show_package(api_client, name_or_id).await,
        PackageAction::Update {
            name,
            sync_interval,
            level,
            description,
        } => update_package(api_client, name, sync_interval, level, description).await,
        PackageAction::Remove { name, confirm } => remove_package(api_client, name, confirm).await,
    }
}

/// 添加软件包
async fn add_package(
    api_client: &ApiClient,
    name: String,
    level: i32,
