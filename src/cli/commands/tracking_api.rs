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

//! 跟踪配置管理命令实现（基于 API）
//
//! 通过 HTTP API 管理跟踪配置

use anyhow::{anyhow, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::cli::client::ApiClient;
use crate::cli::dto::{CreateTrackingRequest, PackageDto, TrackingDto, UpdateTrackingRequest};
use crate::cli::formatter::format_datetime_local;
use crate::cli::parser::TrackingAction;

/// API 响应包装
#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T> {
    data: T,
}

/// 列表响应
#[derive(Debug, Serialize, Deserialize)]
struct ListResponse<T> {
    items: Vec<T>,
    total: usize,
}

/// 解析 "owner/repo"、"owner:repo"、"owner&repo" 或完整 URL
fn parse_owner_repo(input: &str) -> Result<(String, String)> {
    let trimmed = input.trim().trim_matches(|c| c == '\'' || c == '"');

    // URL 形式，例如 https://gitee.com/src-openeuler/elfutils.git
    if trimmed.contains("://") {
        let url_path = trimmed.split("://").nth(1).unwrap_or(trimmed);
        let parts: Vec<&str> = url_path.split('/').collect();
        if parts.len() >= 3 {
            let owner = parts[1];
            let mut repo = parts[2];
            if let Some(stripped) = repo.strip_suffix(".git") {
                repo = stripped;
            }
            return Ok((owner.to_string(), repo.to_string()));
        } else {
            return Err(anyhow!(
                "无效的仓库URL，需形如 https://host/owner/repo(.git)"
            ));
        }
    }

    // 非 URL 形式，支持 / : & 分隔
    let sep = if trimmed.contains('/') {
        '/'
    } else if trimmed.contains(':') {
        ':'
    } else if trimmed.contains('&') {
        '&'
    } else {
        return Err(anyhow!(
            "无效的仓库格式，请使用 owner/repo、owner:repo 或完整URL"
        ));
    };

    let mut iter = trimmed.split(sep);
    let owner = iter.next().unwrap_or_default().to_string();
    let repo = iter.next().unwrap_or_default().to_string();
    if owner.is_empty() || repo.is_empty() {
        return Err(anyhow!(
            "无效的仓库格式，请使用 owner/repo、owner:repo 或完整URL"
        ));
    }
    Ok((owner, repo))
}

/// 解析 distro 参数为数值 ID（目前不支持按名称查询）
fn parse_distro_id(input: &str) -> Result<i32> {
    input
        .parse::<i32>()
        .map_err(|_| anyhow!("发行版未提供查询 API，请使用数值 ID"))
}

/// 解析 package 名称或 ID
async fn resolve_package_id(api_client: &ApiClient, input: &str) -> Result<i32> {
    if let Ok(id) = input.parse::<i32>() {
        return Ok(id);
    }

    // 目前服务端不支持按名称查询，拉取列表后匹配
    let packages: Vec<PackageDto> = api_client.get("/packages").await?;
    if let Some(pkg) = packages.iter().find(|p| p.name == input) {
        Ok(pkg.id)
    } else {
        Err(anyhow!(format!("未找到名称为 '{}' 的软件包", input)))
    }
}

/// 执行跟踪配置管理命令
pub async fn execute(api_client: &ApiClient, action: TrackingAction) -> Result<()> {
    match action {
        TrackingAction::Add {
            package,
            distro,
            l1_repo,
            l1_branch,
            l2_repo,
            l2_branch,
            status,
        } => {
            add_tracking(
                api_client, package, distro, l1_repo, l1_branch, l2_repo, l2_branch, status,
            )
            .await
        }
        TrackingAction::List {
            limit,
            package,
            status,
        } => list_tracking(api_client, limit, package, status).await,
        TrackingAction::Show { id } => show_tracking(api_client, id).await,
        TrackingAction::Pause { id } => update_tracking_status(api_client, id, false).await,
        TrackingAction::Resume { id } => update_tracking_status(api_client, id, true).await,
        TrackingAction::Remove { id, confirm } => remove_tracking(api_client, id, confirm).await,
    }
}

/// 添加跟踪配置
#[allow(clippy::too_many_arguments)]
async fn add_tracking(
    api_client: &ApiClient,
    package: String,
    distro: String,
    l1_repo: String,
    l1_branch: String,
    l2_repo_path: String,
    l2_branch: String,
    status: String,
) -> Result<()> {
    println!("正在添加跟踪配置...");

    let package_id = resolve_package_id(api_client, &package).await?;
    let distro_id = parse_distro_id(&distro)?;
    let (l1_owner, l1_name) = parse_owner_repo(&l1_repo)?;

    let request = CreateTrackingRequest {
        package_id,
        distro_id,
        l1_repo_owner: l1_owner,
        l1_repo_name: l1_name,
        l1_branch,
        l2_branch,
        l2_repo_path,
        tracking_status: Some(status),
    };

    match api_client
        .post::<_, ApiResponse<TrackingDto>>("/tracking", &request)
        .await
    {
        Ok(response) => {
            println!("{} 跟踪配置添加成功", "✓".green().bold());
            println!("  ID: {}", response.data.id);
            println!(
                "  L1 仓库: {}/{} ({})",
                response.data.l1_repo_owner, response.data.l1_repo_name, response.data.l1_branch
            );
            println!(
                "  L2 路径: {} ({})",
                response.data.l2_repo_path, response.data.l2_branch
            );
            println!("  状态: {}", response.data.tracking_status);
            Ok(())
        }
        Err(e) => {
            println!("{} 添加跟踪配置失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 列出跟踪配置
async fn list_tracking(
    api_client: &ApiClient,
    limit: u64,
    package: Option<String>,
    status: Option<String>,
) -> Result<()> {
    println!("正在获取跟踪配置列表...");
