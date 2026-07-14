/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

//! Ecosystem CLI 命令实现（基于 API）

use anyhow::{anyhow, bail, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::client::ApiClient;
use crate::cli::dto::{
    CreateEcosystemTargetRequest, EcosystemRefreshResultDto, EcosystemReportDto,
    EcosystemTargetDto, UpdateEcosystemTargetRequest,
};
use crate::cli::formatter::format_datetime_local;
use crate::cli::parser::EcosystemAction;

#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T> {
    data: Option<T>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PaginatedResponse<T> {
    items: Vec<T>,
    total: u64,
    page: u64,
    page_size: u64,
    total_pages: u64,
}

#[derive(Debug, Clone)]
struct EcosystemPreset {
    canonical_name: String,
    target_type: String,
    platform: Option<String>,
    role: String,
    homepage_url: Option<String>,
    api_base_url: Option<String>,
    owner: Option<String>,
    repo: Option<String>,
    default_branch: Option<String>,
    rule_profile: String,
}

pub async fn execute(api_client: &ApiClient, action: EcosystemAction) -> Result<()> {
    match action {
        EcosystemAction::Create {
            name,
            target_type,
            role,
            rule_profile,
            platform,
            homepage_url,
            api_base_url,
            owner,
            repo,
            default_branch,
            status,
            refresh_interval_hours,
            metadata,
        } => {
            let preset = ecosystem_preset_from_name(&name);
