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
}

fn default_target_type() -> String {
    "community".to_string()
}

fn default_role() -> String {
    "governance".to_string()
}

fn default_rule_profile() -> String {
    "default".to_string()
}

fn default_status() -> String {
    "active".to_string()
}

fn default_refresh_interval_hours() -> i32 {
    24
}

async fn create_target(
    api_client: &ApiClient,
    request: CreateEcosystemTargetRequest,
) -> Result<()> {
    println!("正在创建生态目标: {}", request.name.cyan());
    let response = api_client
        .post::<_, ApiResponse<EcosystemTargetDto>>("/ecosystem/targets", &request)
        .await?;
    let target = response
        .data
        .ok_or_else(|| anyhow!("服务端未返回生态目标数据"))?;
    println!("{} 生态目标创建成功", "✓".green().bold());
    print_target_detail(&target);
    Ok(())
}

async fn list_targets(
    api_client: &ApiClient,
    page: u64,
    page_size: u64,
    target_type: Option<String>,
    platform: Option<String>,
    status: Option<String>,
) -> Result<()> {
    println!("{}", "正在获取生态目标列表...".cyan());
    let mut query = format!("?page={}&page_size={}", page, page_size);
    if let Some(target_type) = target_type {
        query.push_str(&format!("&target_type={}", target_type));
    }
    if let Some(platform) = platform {
        query.push_str(&format!("&platform={}", platform));
    }
    if let Some(status) = status {
        query.push_str(&format!("&status={}", status));
    }

    let response = api_client
        .get::<ApiResponse<PaginatedResponse<EcosystemTargetDto>>>(&format!(
            "/ecosystem/targets{}",
            query
        ))
        .await?;
    let data = response
        .data
        .ok_or_else(|| anyhow!("服务端未返回生态目标列表"))?;

    if data.items.is_empty() {
        println!("{}", "没有找到生态目标".yellow());
        return Ok(());
    }

    println!();
    println!("{}", "生态目标列表:".bold());
    println!(
        "{:<5} {:<24} {:<14} {:<12} {:<14} {:<12}",
        "ID", "名称", "类型", "平台", "规则画像", "状态"
    );
    println!("{}", "-".repeat(90));
    for item in data.items {
        println!(
            "{:<5} {:<24} {:<14} {:<12} {:<14} {:<12}",
            item.id,
            item.name.cyan(),
            item.target_type,
            item.platform.unwrap_or_else(|| "-".to_string()),
            item.rule_profile,
            item.status
        );
    }
    println!(
        "\n共 {} 条，当前第 {}/{} 页",
        data.total, data.page, data.total_pages
    );
    Ok(())
}

async fn show_target(api_client: &ApiClient, id: i32) -> Result<()> {
    println!("正在获取生态目标详情: {}", id.to_string().cyan());
    let response = api_client
        .get::<ApiResponse<EcosystemTargetDto>>(&format!("/ecosystem/targets/{}", id))
        .await?;
    let target = response
        .data
        .ok_or_else(|| anyhow!("服务端未返回生态目标详情"))?;
    print_target_detail(&target);
    Ok(())
}

async fn update_target(
    api_client: &ApiClient,
    target: String,
    request: UpdateEcosystemTargetRequest,
) -> Result<()> {
    let id = resolve_target_id(api_client, &target).await?;
    println!("正在更新生态目标: {}", target.cyan());
    let response = api_client
        .put::<_, ApiResponse<EcosystemTargetDto>>(&format!("/ecosystem/targets/{}", id), &request)
        .await?;
    let target = response
        .data
        .ok_or_else(|| anyhow!("服务端未返回更新后的生态目标"))?;
    println!("{} 生态目标更新成功", "✓".green().bold());
    print_target_detail(&target);
    Ok(())
}

async fn resolve_target_id(api_client: &ApiClient, input: &str) -> Result<i32> {
    if let Ok(id) = input.parse::<i32>() {
        return Ok(id);
    }

    let targets = fetch_all_targets(api_client).await?;
    let input_key = normalize_lookup_key(input);

    if let Some(target) = targets
        .iter()
        .find(|target| normalize_lookup_key(&target.name) == input_key)
    {
        return Ok(target.id);
    }

    let matches = targets
        .iter()
        .filter(|target| {
            let name_key = normalize_lookup_key(&target.name);
            name_key.contains(&input_key) || input_key.contains(&name_key)
        })
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [target] => Ok(target.id),
        [] => bail!("未找到生态目标: {}", input),
        many => {
            let names = many
                .iter()
                .map(|target| format!("{}({})", target.name, target.id))
                .collect::<Vec<_>>()
                .join(", ");
            bail!("匹配到多个生态目标，请使用更精确的名称或 ID: {}", names)
        }
    }
}

async fn fetch_all_targets(api_client: &ApiClient) -> Result<Vec<EcosystemTargetDto>> {
    let mut page = 1u64;
    let page_size = 100u64;
    let mut targets = Vec::new();

    loop {
        let response = api_client
            .get::<ApiResponse<PaginatedResponse<EcosystemTargetDto>>>(&format!(
                "/ecosystem/targets?page={}&page_size={}",
                page, page_size
            ))
            .await?;
        let data = response
            .data
            .ok_or_else(|| anyhow!("服务端未返回生态目标列表"))?;

        let total_pages = data.total_pages;
        targets.extend(data.items);
        if page >= total_pages || total_pages == 0 {
            break;
        }
        page += 1;
    }

    Ok(targets)
}

async fn delete_target(api_client: &ApiClient, id: i32, confirm: bool) -> Result<()> {
    if !confirm {
        bail!("危险操作：删除生态目标需要 --confirm 参数");
    }
    println!("正在删除生态目标: {}", id.to_string().cyan());
    api_client
        .delete_no_content(&format!("/ecosystem/targets/{}", id))
        .await?;
    println!("{} 生态目标删除成功", "✓".green().bold());
    Ok(())
}

async fn refresh_target(api_client: &ApiClient, id: i32) -> Result<()> {
    println!("正在刷新生态目标: {}", id.to_string().cyan());
    let response = api_client
        .post::<_, ApiResponse<EcosystemRefreshResultDto>>(
            &format!("/ecosystem/targets/{}/refresh", id),
            &serde_json::json!({}),
        )
        .await?;
    let result = response
        .data
        .ok_or_else(|| anyhow!("服务端未返回刷新结果"))?;
    println!("{} 生态目标刷新成功", "✓".green().bold());
    println!("  目标 ID: {}", result.target_id);
    println!("  证据数量: {}", result.evidence_count);
    println!("  报告 ID: {}", result.report_id);
    println!(
        "  生成时间: {}",
        format_datetime_local(&result.generated_at)
    );
    Ok(())
}

async fn latest_report(api_client: &ApiClient, id: i32, verbose: bool) -> Result<()> {
    println!("正在获取最新生态报告: {}", id.to_string().cyan());
    let response = api_client
        .get::<ApiResponse<EcosystemReportDto>>(&format!("/ecosystem/targets/{}/latest-report", id))
        .await?;
    let report = response
        .data
        .ok_or_else(|| anyhow!("服务端未返回最新生态报告"))?;
    print_report_detail(&report, verbose);
    Ok(())
}

async fn list_reports(
    api_client: &ApiClient,
    page: u64,
    page_size: u64,
    target_id: Option<i32>,
    report_type: Option<String>,
) -> Result<()> {
    println!("{}", "正在获取生态报告列表...".cyan());
    let mut query = format!("?page={}&page_size={}", page, page_size);
    if let Some(target_id) = target_id {
        query.push_str(&format!("&target_id={}", target_id));
    }
    if let Some(report_type) = report_type {
        query.push_str(&format!("&report_type={}", report_type));
    }
    let response = api_client
        .get::<ApiResponse<PaginatedResponse<EcosystemReportDto>>>(&format!(
            "/ecosystem/reports{}",
            query
        ))
        .await?;
    let data = response
        .data
        .ok_or_else(|| anyhow!("服务端未返回生态报告列表"))?;

    if data.items.is_empty() {
        println!("{}", "没有找到生态报告".yellow());
        return Ok(());
    }

    println!();
    println!("{}", "生态报告列表:".bold());
    println!(
        "{:<8} {:<8} {:<20} {:<12} {:<12} {:<20}",
        "ID", "目标ID", "类型", "风险", "置信度", "生成时间"
    );
    println!("{}", "-".repeat(90));
    for item in data.items {
        println!(
            "{:<8} {:<8} {:<20} {:<12} {:<12} {:<20}",
            item.id,
            item.target_id,
            item.report_type,
            item.overall_risk,
            item.confidence,
            format_datetime_local(&item.generated_at)
        );
    }
    println!(
        "\n共 {} 条，当前第 {}/{} 页",
        data.total, data.page, data.total_pages
    );
    Ok(())
}

async fn show_report(api_client: &ApiClient, id: i64, verbose: bool) -> Result<()> {
    println!("正在获取生态报告详情: {}", id.to_string().cyan());
    let response = api_client
        .get::<ApiResponse<EcosystemReportDto>>(&format!("/ecosystem/reports/{}", id))
        .await?;
    let report = response
        .data
        .ok_or_else(|| anyhow!("服务端未返回生态报告详情"))?;
    print_report_detail(&report, verbose);
    Ok(())
}

fn print_target_detail(target: &EcosystemTargetDto) {
    println!();
    println!("{}", "生态目标详情:".bold());
    println!("  ID: {}", target.id);
    println!("  名称: {}", target.name.cyan());
    println!("  类型: {}", target.target_type);
    println!(
        "  平台: {}",
        target.platform.clone().unwrap_or_else(|| "-".to_string())
    );
    println!("  角色: {}", target.role);
    println!("  规则画像: {}", target.rule_profile);
    println!("  状态: {}", target.status);
    println!("  刷新间隔: {} 小时", target.refresh_interval_hours);
    if let Some(homepage) = &target.homepage_url {
        println!("  首页: {}", homepage);
    }
    if let Some(api_base) = &target.api_base_url {
        println!("  API 地址: {}", api_base);
    }
    if let Some(owner) = &target.owner {
        println!("  Owner: {}", owner);
    }
    if let Some(repo) = &target.repo {
        println!("  Repo: {}", repo);
    }
    if let Some(branch) = &target.default_branch {
        println!("  默认分支: {}", branch);
    }
    println!(
        "  最近采集: {}",
        target
            .last_collected_at
            .as_ref()
            .map(format_datetime_local)
            .unwrap_or_else(|| "-".to_string())
    );
    println!(
        "  最近报告: {}",
        target
            .last_report_at
            .as_ref()
            .map(format_datetime_local)
            .unwrap_or_else(|| "-".to_string())
    );
    if let Some(last_error) = &target.last_error {
        println!("  最近错误: {}", last_error.red());
    }
    if let Some(metadata) = &target.metadata {
        println!("  Metadata: {}", metadata);
    }
    println!("  创建时间: {}", format_datetime_local(&target.created_at));
    println!("  更新时间: {}", format_datetime_local(&target.updated_at));
}

fn print_report_detail(report: &EcosystemReportDto, verbose: bool) {
    println!();
    println!("{}", "生态报告详情:".bold());
    println!("  ID: {}", report.id);
    println!("  目标 ID: {}", report.target_id);
    println!("  报告类型: {}", report.report_type);
    println!("  状态: {}", report.status);
    println!("  综合风险: {}", report.overall_risk);
    println!("  置信度: {}", report.confidence);
    println!("  摘要: {}", report.summary);
    print_source_focus_details(&report.report_payload);
    println!("  维度摘要: {}", report.dimensions);
    if verbose {
        if let Some(evidence_summary) = &report.evidence_summary {
            println!("  证据摘要: {}", evidence_summary);
        }
        println!("  报告载荷: {}", report.report_payload);
    } else if let Some(evidence_summary) = &report.evidence_summary {
        println!("  证据摘要: {}", evidence_summary);
    }
    println!(
        "  生成时间: {}",
        format_datetime_local(&report.generated_at)
    );
}

fn print_source_focus_details(report_payload: &Value) {
    let organization = find_source_focus_value(report_payload, "organization_structure");
    let foundation = find_source_focus_value(report_payload, "foundation_status");
    let operator_name = find_source_focus_value(report_payload, "operator_name");
    let operator_supply_risk = find_source_focus_value(report_payload, "operator_supply_risk");
    let lifecycle = find_source_focus_value(report_payload, "version_lifecycle");
    let platform_intro = find_source_focus_value(report_payload, "platform_intro");
    let trade_controls = find_source_focus_value(report_payload, "trade_controls");
    let ip_policy = find_source_focus_value(report_payload, "ip_policy");
    let government_takedown = find_source_focus_value(report_payload, "government_takedown_policy");
    let license = find_source_focus_value(report_payload, "license_policy");
    let copyright = find_source_focus_value(report_payload, "copyright_info");
    let cla = find_source_focus_value(report_payload, "cla_policy");
    let hash_signature = find_quality_focus_value(report_payload, "hash_signature_assessment");

    if organization.is_none()
        && foundation.is_none()
        && operator_name.is_none()
        && operator_supply_risk.is_none()
        && lifecycle.is_none()
        && platform_intro.is_none()
        && trade_controls.is_none()
        && ip_policy.is_none()
        && government_takedown.is_none()
        && license.is_none()
        && copyright.is_none()
        && cla.is_none()
        && hash_signature.is_none()
    {
        return;
    }

    println!("  来源重点信息:");
    if let Some(platform_intro) = platform_intro {
        println!("    - 平台简介: {}", platform_intro);
    }
    println!(
        "    - 组织架构: {}",
        organization.unwrap_or_else(|| "-".to_string())
    );
    println!(
        "    - 基金会信息: {}",
        foundation.unwrap_or_else(|| "-".to_string())
    );
    println!(
        "    - 运营方: {}",
        operator_name.unwrap_or_else(|| "-".to_string())
    );
    println!(
        "    - 版本生命周期: {}",
        lifecycle.unwrap_or_else(|| "-".to_string())
    );
    print_lifecycle_structured_details(report_payload);
    if let Some(trade_controls) = trade_controls {
        println!("    - 贸易管制情况: {}", trade_controls);
    }
    print_github_trade_controls_details(report_payload);
    if let Some(ip_policy) = ip_policy {
        println!("    - 知识产权情况: {}", ip_policy);
    }
    print_github_ip_policy_details(report_payload);
    if let Some(government_takedown) = government_takedown {
        println!("    - 政府下架情况: {}", government_takedown);
    }
    print_github_government_takedown_details(report_payload);
    print_github_gov_takedown_archive_details(report_payload);
    println!(
        "    - 许可证信息: {}",
        license.unwrap_or_else(|| "-".to_string())
    );
    print_github_license_policy_details(report_payload);
    if let Some(copyright) = copyright {
        println!("    - Copyright 信息: {}", copyright);
    }
    print_github_copyright_details(report_payload);
    println!("    - CLA 信息: {}", cla.unwrap_or_else(|| "-".to_string()));
    if let Some(operator_supply_risk) = operator_supply_risk {
        println!("    - 运营方供应风险: {}", operator_supply_risk);
    }
    if let Some(hash_signature) = hash_signature {
        println!("    - 哈希/签名机制: {}", hash_signature);
    }
    print_download_integrity_details(report_payload);
}

fn print_lifecycle_structured_details(report_payload: &Value) {
    let mut details = Vec::new();

    if source_focus_bool(report_payload, "has_lts_policy") == Some(true) {
        details.push("      * 版本分类: 区分 LTS 版本和创新版本".to_string());
    }
    if source_focus_bool(report_payload, "lts_every_four_years") == Some(true) {
        details.push("      * LTS 发布周期: 自 2025 年 8 月起每 4 年发布一代".to_string());
    } else if source_focus_bool(report_payload, "lts_every_two_years") == Some(true) {
        details.push("      * LTS 发布周期: 历史规则约每 2 年发布一代".to_string());
    }
    if source_focus_bool(report_payload, "lts_support_four_years") == Some(true) {
        details.push("      * LTS 社区支持: 4 年".to_string());
    }
    if source_focus_bool(report_payload, "lts_lifecycle_six_years") == Some(true) {
        details.push("      * LTS 全生命周期: 6 年（4+2）".to_string());
    }
    if source_focus_bool(report_payload, "lts_extendable_to_eight_years") == Some(true) {
        details.push("      * 生命周期延长: 可申请延长至 8 年".to_string());
    }
    if source_focus_bool(report_payload, "innovation_every_twelve_months") == Some(true) {
        details.push("      * 创新版发布周期: 每 12 个月发布一次".to_string());
    } else if source_focus_bool(report_payload, "innovation_every_six_months") == Some(true) {
        details.push("      * 创新版发布周期: 历史规则约每 6 个月发布一次".to_string());
    }
    if source_focus_bool(report_payload, "innovation_support_six_months") == Some(true) {
        details.push("      * 创新版社区支持: 6 个月".to_string());
    }
    if source_focus_bool(report_payload, "sp_policy_mentioned") == Some(true) {
        details.push("      * SP 生命周期策略: 按大小 SP 区分维护周期".to_string());
    }
    if source_focus_bool(report_payload, "extended_support_mentioned") == Some(true) {
        details.push("      * 扩展支持: -".to_string());
    }

    if details.is_empty() {
        return;
    }

    println!("      具体规则:");
    for detail in details {
        println!("{}", detail);
    }
}

fn print_github_trade_controls_details(report_payload: &Value) {
    let mut details = Vec::new();

    if source_focus_bool(report_payload, "ofac_license_for_iran") == Some(true) {
        details.push("      * OFAC 许可: 已公开说明伊朗开发者云服务许可".to_string());
    }
    if source_focus_bool(report_payload, "public_repo_access_in_sanctioned_regions") == Some(true) {
        details.push("      * 开源访问: 在部分受制裁地区维持公共仓库/开源协作访问".to_string());
    }
    if source_focus_bool(report_payload, "itar_restriction_mentioned") == Some(true) {
        details.push("      * ITAR 限制: GitHub.com 不适合托管 ITAR 受控数据".to_string());
    }
    if source_focus_bool(report_payload, "restricted_regions_mentioned") == Some(true) {
        details.push("      * 受限范围: 涉及受制裁国家/地区及被拒绝方访问限制".to_string());
    }

    print_structured_details_block("      贸易管制字段:", details);
}

fn print_github_ip_policy_details(report_payload: &Value) {
    let mut details = Vec::new();

    if source_focus_bool(report_payload, "users_own_content") == Some(true) {
        details.push("      * 内容归属: 用户保有其发布内容的所有权".to_string());
    }
    if source_focus_bool(report_payload, "license_grant_to_host_content") == Some(true) {
        details
            .push("      * 平台授权: 用户需授予 GitHub 托管、展示、解析内容的必要许可".to_string());
    }
    if source_focus_bool(report_payload, "github_retains_platform_ip") == Some(true) {
        details.push(
            "      * 平台知识产权: 网站、服务及界面相关知识产权由 GitHub 及其许可方保留"
                .to_string(),
        );
    }

    print_structured_details_block("      知识产权字段:", details);
}

fn print_github_government_takedown_details(report_payload: &Value) {
    let mut details = Vec::new();

    if source_focus_bool(report_payload, "supports_geographic_limit") == Some(true) {
        details.push("      * 处置范围: 优先按地理范围限制下架影响面".to_string());
    }
    if source_focus_bool(report_payload, "supports_user_appeal") == Some(true) {
        details.push("      * 用户救济: 允许受影响用户申诉".to_string());
    }
    if source_focus_bool(report_payload, "publishes_public_requests") == Some(true) {
        details.push("      * 透明度: 官方请求会公开到 `gov-takedowns` 仓库".to_string());
    }

    print_structured_details_block("      政府下架字段:", details);
}

fn print_github_gov_takedown_archive_details(report_payload: &Value) {
    // archive_error 为 null 表示无错误，为 String 表示采集失败 → 跳过展示
    let archive_error = find_source_raw_evidence_json_value(report_payload, "archive_error");
    if matches!(&archive_error, Some(v) if v.is_string()) {
        return;
    }

    let total = match find_source_raw_evidence_json_value(report_payload, "total_requests")
        .and_then(|v| v.as_u64())
    {
        Some(n) => n,
        None => return,
    };

    let truncated = find_source_raw_evidence_json_value(report_payload, "truncated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let by_requester = find_source_raw_evidence_json_value(report_payload, "requests_by_requester");

    let trunc_note = if truncated {
        "（列表已截断，实际更多）"
    } else {
        ""
    };
    println!("      政府下架档案 (github/gov-takedowns):{}", trunc_note);
    println!("        * 历史请求总数: {} 条", total);

    if let Some(serde_json::Value::Object(map)) = by_requester {
        let mut entries: Vec<(String, u64)> = map
            .into_iter()
            .filter_map(|(k, v)| v.as_u64().map(|n| (k, n)))
            .collect();
        // 按请求数降序，同数量时按名称字母序
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        if !entries.is_empty() {
            println!("        * 请求方明细 (按数量降序):");
            for (requester, count) in &entries {
                println!("            - {}: {} 条", requester, count);
            }
        }
    }
}

fn print_github_license_policy_details(report_payload: &Value) {
    let mut details = Vec::new();

    if source_focus_bool(report_payload, "supports_choosealicense") == Some(true) {
        details.push("      * 许可证选型: 提供 Choose a License 指引".to_string());
    }
    if source_focus_bool(report_payload, "supports_license_detection") == Some(true) {
        details.push("      * 许可证识别: 支持 Licensee / Licenses API 等识别能力".to_string());
    }
    if source_focus_bool(report_payload, "mentions_default_copyright_rule") == Some(true) {
        details.push("      * 默认规则: 未声明许可证时默认版权法仍然适用".to_string());
    }

    print_structured_details_block("      许可证字段:", details);
}

fn print_github_copyright_details(report_payload: &Value) {
    let mut details = Vec::new();

    if source_focus_bool(report_payload, "dmca_safe_harbor_mentioned") == Some(true) {
        details.push("      * DMCA Safe Harbor: 平台明确维持 DMCA 安全港合规".to_string());
    }
    if source_focus_bool(report_payload, "counter_notice_supported") == Some(true) {
        details.push("      * 反通知机制: 支持 counter notice 流程".to_string());
    }
    if source_focus_bool(report_payload, "github_copyright_notice_mentioned") == Some(true) {
        details.push("      * 平台版权声明: GitHub 网站与服务外观受 GitHub 版权保护".to_string());
    }

    print_structured_details_block("      Copyright 字段:", details);
}

fn print_download_integrity_details(report_payload: &Value) {
    let mut details = Vec::new();

    if quality_focus_bool(report_payload, "supports_gpg_commit_tag_verification") == Some(true) {
        details.push("      * GPG 验签: 支持提交/Tag 的 GPG 签名与验签".to_string());
    }
    if quality_focus_bool(report_payload, "supports_release_attachments") == Some(true) {
        details.push("      * 发布下载: 支持 Release/附件/源码下载".to_string());
    }
    if quality_focus_bool(report_payload, "hash_verification_supported") == Some(false) {
        details.push("      * 哈希校验: 未检索到平台公开提供的 Release 校验值机制".to_string());
    }
    if quality_focus_bool(report_payload, "documented_release_artifact_signature") == Some(false) {
        details.push("      * 下载物签名: 未检索到公开的 Release 附件数字签名文档".to_string());
    }
    if quality_focus_bool(report_payload, "provenance_attestation") == Some(false) {
        details.push("      * 来源证明: 未检索到公开的 provenance/attestation 文档".to_string());
    }

    print_structured_details_block("      哈希/签名字段:", details);
}

fn print_structured_details_block(title: &str, details: Vec<String>) {
    if details.is_empty() {
        return;
    }

    println!("{}", title);
    for detail in details {
        println!("{}", detail);
    }
}

fn find_source_focus_value(report_payload: &Value, key: &str) -> Option<String> {
    find_source_indicator_value(report_payload, key)
        .or_else(|| find_source_raw_evidence_value(report_payload, key))
}

fn find_quality_focus_value(report_payload: &Value, key: &str) -> Option<String> {
    find_quality_indicator_value(report_payload, key)
        .or_else(|| find_quality_raw_evidence_value(report_payload, key))
}

fn source_focus_bool(report_payload: &Value, key: &str) -> Option<bool> {
    find_source_indicator_json_value(report_payload, key)
        .or_else(|| find_source_raw_evidence_json_value(report_payload, key))
        .and_then(|value| match value {
            Value::Bool(flag) => Some(flag),
            Value::String(text) => match text.to_ascii_lowercase().as_str() {
                "true" | "yes" | "1" => Some(true),
                "false" | "no" | "0" => Some(false),
                _ => None,
            },
            _ => None,
        })
}

fn quality_focus_bool(report_payload: &Value, key: &str) -> Option<bool> {
    find_quality_indicator_json_value(report_payload, key)
        .or_else(|| find_quality_raw_evidence_json_value(report_payload, key))
        .and_then(|value| match value {
            Value::Bool(flag) => Some(flag),
            Value::String(text) => match text.to_ascii_lowercase().as_str() {
                "true" | "yes" | "1" => Some(true),
                "false" | "no" | "0" => Some(false),
                _ => None,
            },
            _ => None,
        })
}

fn find_source_indicator_value(report_payload: &Value, key: &str) -> Option<String> {
    find_source_indicator_json_value(report_payload, key)
        .and_then(|value| value_to_readable_text(&value))
}

fn find_quality_indicator_value(report_payload: &Value, key: &str) -> Option<String> {
    find_quality_indicator_json_value(report_payload, key)
        .and_then(|value| value_to_readable_text(&value))
}

fn find_source_indicator_json_value(report_payload: &Value, key: &str) -> Option<Value> {
    report_payload
        .get("sections")
        .and_then(|sections| sections.get("source"))
        .and_then(|source| source.get("indicators"))
        .and_then(Value::as_array)
        .and_then(|indicators| {
            indicators.iter().find_map(|indicator| {
                let indicator_key = indicator.get("key").and_then(Value::as_str)?;
                if indicator_key == key {
                    indicator.get("value").cloned()
                } else {
                    None
                }
            })
        })
}

fn find_quality_indicator_json_value(report_payload: &Value, key: &str) -> Option<Value> {
    report_payload
        .get("sections")
        .and_then(|sections| sections.get("quality"))
        .and_then(|quality| quality.get("indicators"))
        .and_then(Value::as_array)
        .and_then(|indicators| {
            indicators.iter().find_map(|indicator| {
                let indicator_key = indicator.get("key").and_then(Value::as_str)?;
                if indicator_key == key {
                    indicator.get("value").cloned()
                } else {
                    None
                }
            })
        })
}

fn find_source_raw_evidence_value(report_payload: &Value, key: &str) -> Option<String> {
    find_source_raw_evidence_json_value(report_payload, key)
        .and_then(|value| value_to_readable_text(&value))
}

fn find_source_raw_evidence_json_value(report_payload: &Value, key: &str) -> Option<Value> {
    report_payload
        .get("raw_evidence")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries.iter().find_map(|entry| {
                let category = entry.get("assessment_category").and_then(Value::as_str)?;
                if category != "source" {
                    return None;
                }
                entry.get("data").and_then(|data| data.get(key)).cloned()
            })
        })
}

fn find_quality_raw_evidence_value(report_payload: &Value, key: &str) -> Option<String> {
    find_quality_raw_evidence_json_value(report_payload, key)
        .and_then(|value| value_to_readable_text(&value))
}

fn find_quality_raw_evidence_json_value(report_payload: &Value, key: &str) -> Option<Value> {
    report_payload
        .get("raw_evidence")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries.iter().find_map(|entry| {
                let category = entry.get("assessment_category").and_then(Value::as_str)?;
                if category != "quality" {
                    return None;
                }
                entry.get("data").and_then(|data| data.get(key)).cloned()
            })
        })
}

fn value_to_readable_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Array(items) => {
            if items.is_empty() {
                return None;
            }
            let values = items
                .iter()
                .filter_map(value_to_readable_text)
                .collect::<Vec<_>>();
            if values.is_empty() {
                None
            } else {
                Some(values.join("；"))
            }
        }
        Value::Object(_) => serde_json::to_string(value).ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_metadata_accepts_valid_json() {
        let value = parse_metadata(Some(
            r#"{"source_assessment":{"foundation":{"a":1}}}"#.to_string(),
        ))
        .unwrap();
        assert!(value.is_some());
    }

    #[test]
    fn parse_metadata_rejects_invalid_json() {
        let err = parse_metadata(Some("{invalid}".to_string())).unwrap_err();
        assert!(err.to_string().contains("metadata 不是合法 JSON"));
    }

    #[test]
    fn ecosystem_create_defaults_are_stable() {
        assert_eq!(default_target_type(), "community");
        assert_eq!(default_role(), "governance");
        assert_eq!(default_rule_profile(), "default");
        assert_eq!(default_status(), "active");
        assert_eq!(default_refresh_interval_hours(), 24);
    }

    #[test]
    fn normalize_lookup_key_ignores_case_and_spaces() {
        assert_eq!(
            normalize_lookup_key("openEuler Community"),
            "openeulercommunity"
        );
        assert_eq!(normalize_lookup_key("OpenEuler"), "openeuler");
    }

    #[test]
    fn ecosystem_preset_recognizes_openeuler_aliases() {
        for input in ["openeuler", "OpenEuler", "openEuler Community"] {
            let preset = ecosystem_preset_from_name(input).expect("preset should exist");
            assert_eq!(preset.canonical_name, "openEuler Community");
            assert_eq!(preset.platform.as_deref(), Some("openeuler"));
            assert_eq!(preset.rule_profile, "openeuler_community");
        }
    }

    #[test]
    fn find_source_focus_value_prefers_indicator_and_fallbacks_to_raw_evidence() {
        let payload = serde_json::json!({
            "sections": {
                "source": {
                    "indicators": [
                        {"key": "organization_structure", "value": "委员会 + SIG"},
                        {"key": "foundation_status", "value": "开放原子开源基金会"},
                        {"key": "cla_policy", "value": "贡献前需要签署 CLA"}
                    ]
                }
            },
            "raw_evidence": [
                {
                    "assessment_category": "source",
                    "data": {
                        "version_lifecycle": "LTS 两年一发，四年支持",
                        "license_policy": "Mulan PSL v2"
                    }
                }
            ]
        });

        assert_eq!(
            find_source_focus_value(&payload, "organization_structure").as_deref(),
            Some("委员会 + SIG")
        );
        assert_eq!(
            find_source_focus_value(&payload, "version_lifecycle").as_deref(),
            Some("LTS 两年一发，四年支持")
        );
        assert_eq!(
            find_source_focus_value(&payload, "license_policy").as_deref(),
            Some("Mulan PSL v2")
        );
    }

    #[test]
    fn ecosystem_preset_recognizes_github_aliases() {
        for input in ["github", "GitHub", "github platform"] {
            let preset = ecosystem_preset_from_name(input).expect("preset should exist");
            assert_eq!(preset.canonical_name, "GitHub Platform");
            assert_eq!(preset.platform.as_deref(), Some("github"));
            assert_eq!(preset.rule_profile, "github_platform");
        }
    }

    #[test]
    fn ecosystem_preset_recognizes_atomgit_aliases() {
        for input in ["atomgit", "AtomGit", "gitcode", "atomgit platform"] {
            let preset = ecosystem_preset_from_name(input).expect("preset should exist");
            assert_eq!(preset.canonical_name, "AtomGit Platform");
            assert_eq!(preset.platform.as_deref(), Some("atomgit"));
            assert_eq!(preset.rule_profile, "atomgit_platform");
        }
    }

    #[test]
    fn source_focus_bool_reads_indicator_and_raw_evidence() {
        let payload = serde_json::json!({
            "sections": {
                "source": {
                    "indicators": [
                        {"key": "has_lts_policy", "value": true}
                    ]
                }
            },
            "raw_evidence": [
                {
                    "assessment_category": "source",
                    "data": {
                        "innovation_support_six_months": true
                    }
                }
            ]
        });

        assert_eq!(source_focus_bool(&payload, "has_lts_policy"), Some(true));
        assert_eq!(
            source_focus_bool(&payload, "innovation_support_six_months"),
            Some(true)
        );
    }

    #[test]
    fn lifecycle_structured_flags_can_be_extracted() {
        let payload = serde_json::json!({
            "sections": {
                "source": {
                    "indicators": [
                        {"key": "has_lts_policy", "value": true},
                        {"key": "lts_every_four_years", "value": true},
                        {"key": "lts_support_four_years", "value": true},
                        {"key": "lts_lifecycle_six_years", "value": true},
                        {"key": "innovation_every_twelve_months", "value": true},
                        {"key": "innovation_support_six_months", "value": true},
                        {"key": "sp_policy_mentioned", "value": true},
                        {"key": "extended_support_mentioned", "value": true}
                    ]
                }
            }
        });

        assert_eq!(
            source_focus_bool(&payload, "lts_every_four_years"),
            Some(true)
        );
        assert_eq!(
            source_focus_bool(&payload, "innovation_every_twelve_months"),
            Some(true)
        );
        assert_eq!(
            source_focus_bool(&payload, "sp_policy_mentioned"),
            Some(true)
        );
        assert_eq!(
            source_focus_bool(&payload, "extended_support_mentioned"),
            Some(true)
        );
    }

    #[test]
    fn github_structured_flags_can_be_extracted() {
        let payload = serde_json::json!({
            "sections": {
                "source": {
                    "indicators": [
                        {"key": "ofac_license_for_iran", "value": true},
                        {"key": "public_repo_access_in_sanctioned_regions", "value": true},
                        {"key": "itar_restriction_mentioned", "value": true},
                        {"key": "restricted_regions_mentioned", "value": true},
                        {"key": "users_own_content", "value": true},
                        {"key": "license_grant_to_host_content", "value": true},
                        {"key": "github_retains_platform_ip", "value": true},
                        {"key": "supports_geographic_limit", "value": true},
                        {"key": "supports_user_appeal", "value": true},
                        {"key": "publishes_public_requests", "value": true},
                        {"key": "supports_choosealicense", "value": true},
                        {"key": "supports_license_detection", "value": true},
                        {"key": "mentions_default_copyright_rule", "value": true},
                        {"key": "dmca_safe_harbor_mentioned", "value": true},
                        {"key": "counter_notice_supported", "value": true},
                        {"key": "github_copyright_notice_mentioned", "value": true}
                    ]
                }
            }
        });

        assert_eq!(
            source_focus_bool(&payload, "ofac_license_for_iran"),
            Some(true)
        );
        assert_eq!(
            source_focus_bool(&payload, "github_retains_platform_ip"),
            Some(true)
        );
        assert_eq!(
            source_focus_bool(&payload, "publishes_public_requests"),
            Some(true)
        );
        assert_eq!(
            source_focus_bool(&payload, "supports_license_detection"),
            Some(true)
        );
        assert_eq!(
            source_focus_bool(&payload, "counter_notice_supported"),
            Some(true)
        );
    }
}
