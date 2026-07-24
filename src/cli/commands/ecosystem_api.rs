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
            let request = CreateEcosystemTargetRequest {
                name: preset
                    .as_ref()
                    .map(|preset| preset.canonical_name.clone())
                    .unwrap_or(name),
                target_type: target_type
                    .or_else(|| preset.as_ref().map(|preset| preset.target_type.clone()))
                    .unwrap_or_else(default_target_type),
                platform: platform
                    .or_else(|| preset.as_ref().and_then(|preset| preset.platform.clone())),
                role: role
                    .or_else(|| preset.as_ref().map(|preset| preset.role.clone()))
                    .unwrap_or_else(default_role),
                homepage_url: homepage_url.or_else(|| {
                    preset
                        .as_ref()
                        .and_then(|preset| preset.homepage_url.clone())
                }),
                api_base_url: api_base_url.or_else(|| {
                    preset
                        .as_ref()
                        .and_then(|preset| preset.api_base_url.clone())
                }),
                owner: owner.or_else(|| preset.as_ref().and_then(|preset| preset.owner.clone())),
                repo: repo.or_else(|| preset.as_ref().and_then(|preset| preset.repo.clone())),
                default_branch: default_branch.or_else(|| {
                    preset
                        .as_ref()
                        .and_then(|preset| preset.default_branch.clone())
                }),
                status: Some(status.unwrap_or_else(default_status)),
                refresh_interval_hours: Some(
                    refresh_interval_hours.unwrap_or_else(default_refresh_interval_hours),
                ),
                rule_profile: rule_profile
                    .or_else(|| preset.as_ref().map(|preset| preset.rule_profile.clone()))
                    .unwrap_or_else(default_rule_profile),
                metadata: parse_metadata(metadata)?,
            };
            create_target(api_client, request).await
        }
        EcosystemAction::List {
            page,
            page_size,
            target_type,
            platform,
            status,
        } => list_targets(api_client, page, page_size, target_type, platform, status).await,
        EcosystemAction::Show { id } => show_target(api_client, id).await,
        EcosystemAction::Update {
            target,
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
            last_error,
        } => {
            let normalized_name = name.map(normalize_create_name);
            let preset = normalized_name
                .as_deref()
                .and_then(ecosystem_preset_from_name);
            let request = UpdateEcosystemTargetRequest {
                name: normalized_name,
                target_type: target_type
                    .or_else(|| preset.as_ref().map(|preset| preset.target_type.clone())),
                platform: platform
                    .or_else(|| preset.as_ref().and_then(|preset| preset.platform.clone())),
                role: role.or_else(|| preset.as_ref().map(|preset| preset.role.clone())),
                homepage_url: homepage_url.or_else(|| {
                    preset
                        .as_ref()
                        .and_then(|preset| preset.homepage_url.clone())
                }),
                api_base_url: api_base_url.or_else(|| {
                    preset
                        .as_ref()
                        .and_then(|preset| preset.api_base_url.clone())
                }),
                owner: owner.or_else(|| preset.as_ref().and_then(|preset| preset.owner.clone())),
                repo: repo.or_else(|| preset.as_ref().and_then(|preset| preset.repo.clone())),
                default_branch: default_branch.or_else(|| {
                    preset
                        .as_ref()
                        .and_then(|preset| preset.default_branch.clone())
                }),
                status,
                refresh_interval_hours,
                rule_profile: rule_profile
                    .or_else(|| preset.as_ref().map(|preset| preset.rule_profile.clone())),
                metadata: parse_metadata(metadata)?,
                last_error,
            };
            update_target(api_client, target, request).await
        }
        EcosystemAction::Delete { id, confirm } => delete_target(api_client, id, confirm).await,
        EcosystemAction::Refresh { id } => refresh_target(api_client, id).await,
        EcosystemAction::LatestReport { id, verbose } => {
            latest_report(api_client, id, verbose).await
        }
        EcosystemAction::Reports {
            page,
            page_size,
            target_id,
            report_type,
        } => list_reports(api_client, page, page_size, target_id, report_type).await,
        EcosystemAction::Report { id, verbose } => show_report(api_client, id, verbose).await,
    }
}

fn parse_metadata(input: Option<String>) -> Result<Option<Value>> {
    match input {
        Some(raw) => {
            let value = serde_json::from_str::<Value>(&raw)
                .map_err(|e| anyhow!("metadata 不是合法 JSON: {}", e))?;
            Ok(Some(value))
        }
        None => Ok(None),
    }
}

fn normalize_lookup_key(input: &str) -> String {
    input
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn normalize_create_name(input: String) -> String {
    ecosystem_preset_from_name(&input)
        .map(|preset| preset.canonical_name)
        .unwrap_or(input)
}

fn ecosystem_preset_from_name(input: &str) -> Option<EcosystemPreset> {
    let key = normalize_lookup_key(input);
    if key.contains("openeuler") {
        return Some(EcosystemPreset {
            canonical_name: "openEuler Community".to_string(),
            target_type: "community".to_string(),
            platform: Some("openeuler".to_string()),
            role: "governance".to_string(),
            homepage_url: Some("https://www.openeuler.org/en/".to_string()),
            api_base_url: Some("https://gitee.com/api/v5".to_string()),
            owner: Some("openeuler".to_string()),
            repo: Some("community".to_string()),
            default_branch: Some("master".to_string()),
            rule_profile: "openeuler_community".to_string(),
        });
    }
    if key == "github" || key.contains("githubplatform") || key.contains("githubcommunity") {
        return Some(EcosystemPreset {
            canonical_name: "GitHub Platform".to_string(),
            target_type: "platform".to_string(),
            platform: Some("github".to_string()),
            role: "hosting".to_string(),
            homepage_url: Some("https://github.com/about".to_string()),
            api_base_url: Some("https://api.github.com".to_string()),
            owner: None,
            repo: None,
            default_branch: None,
            rule_profile: "github_platform".to_string(),
        });
    }
    if key == "atomgit"
        || key == "gitcode"
        || key.contains("atomgitplatform")
        || key.contains("gitcodeplatform")
    {
        return Some(EcosystemPreset {
            canonical_name: "AtomGit Platform".to_string(),
            target_type: "platform".to_string(),
            platform: Some("atomgit".to_string()),
            role: "hosting".to_string(),
            homepage_url: Some("https://atomgit.com".to_string()),
            api_base_url: Some("https://api.atomgit.com/api/v5".to_string()),
            owner: None,
            repo: None,
            default_branch: None,
            rule_profile: "atomgit_platform".to_string(),
        });
    }
    None
