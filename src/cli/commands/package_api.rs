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
