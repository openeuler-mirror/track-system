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

    let mut query = format!("?page=1&page_size={}", limit);

    if let Some(pkg) = package {
        let pkg_id = resolve_package_id(api_client, &pkg).await?;
        query.push_str(&format!("&package_id={}", pkg_id));
    }
    if let Some(st) = status {
        query.push_str(&format!("&tracking_status={}", st));
    }

    match api_client
        .get::<ApiResponse<ListResponse<TrackingDto>>>(&format!("/tracking{}", query))
        .await
    {
        Ok(response) => {
            let trackings = response.data.items;

            if trackings.is_empty() {
                println!("{}", "没有找到跟踪配置".yellow());
                return Ok(());
            }

            println!();
            println!("{}", "跟踪配置列表:".bold());
            println!(
                "{:<5} {:<15} {:<15} {:<30} {:<10}",
                "ID", "软件包ID", "发行版ID", "L1 仓库", "状态"
            );
            println!("{}", "-".repeat(75));

            for track in trackings {
                let l1_repo = format!("{}/{}", track.l1_repo_owner, track.l1_repo_name);
                println!(
                    "{:<5} {:<15} {:<15} {:<30} {:<10}",
                    track.id, track.package_id, track.distro_id, l1_repo, track.tracking_status
                );
            }

            println!();
            println!("总计: {} 个跟踪配置", response.data.total);
            Ok(())
        }
        Err(e) => {
            println!("{} 获取跟踪配置列表失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 显示跟踪配置详情
async fn show_tracking(api_client: &ApiClient, id: i32) -> Result<()> {
    println!("正在获取跟踪配置详情: {}", id);

    match api_client
        .get::<ApiResponse<TrackingDto>>(&format!("/tracking/{}", id))
        .await
    {
        Ok(response) => {
            let track = response.data;
            println!();
            println!("{}", "跟踪配置详情:".bold());
            println!("  ID: {}", track.id);
            println!("  软件包 ID: {}", track.package_id);
            println!("  发行版 ID: {}", track.distro_id);
            println!("  L1 仓库: {}/{}", track.l1_repo_owner, track.l1_repo_name);
            println!("  L1 分支: {}", track.l1_branch);
            println!("  L2 路径: {}", track.l2_repo_path);
            println!("  L2 分支: {}", track.l2_branch);
            println!("  状态: {}", track.tracking_status);
            if let Some(dt) = track.last_sync_time {
                println!("  最近同步: {}", format_datetime_local(&dt));
            }
            if let Some(sha) = track.last_l1_commit_sha {
                println!("  最近 L1 提交: {}", sha);
            }
            if let Some(sha) = track.last_l2_commit_sha {
                println!("  最近 L2 提交: {}", sha);
            }
            println!("  创建时间: {}", format_datetime_local(&track.created_at));
            println!("  更新时间: {}", format_datetime_local(&track.updated_at));
            Ok(())
        }
        Err(e) => {
            println!("{} 获取跟踪配置详情失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 更新跟踪配置状态
async fn update_tracking_status(api_client: &ApiClient, id: i32, enabled: bool) -> Result<()> {
    let action = if enabled { "恢复" } else { "暂停" };
    println!("正在{}跟踪配置: {}", action, id);

    let request = UpdateTrackingRequest {
        l1_repo_owner: None,
        l1_repo_name: None,
        l1_branch: None,
        l2_branch: None,
        l2_repo_path: None,
        tracking_status: Some(if enabled {
            "active".to_string()
        } else {
            "paused".to_string()
        }),
    };

    match api_client
        .put::<_, ApiResponse<TrackingDto>>(&format!("/tracking/{}", id), &request)
        .await
    {
        Ok(_) => {
            println!("{} 跟踪配置{}成功", "✓".green().bold(), action);
            Ok(())
        } 
        Err(e) => {
            println!("{} {}跟踪配置失败: {}", "✗".red().bold(), action, e);
            Err(e.into())
        }
    }
}

/// 删除跟踪配置
async fn remove_tracking(api_client: &ApiClient, id: i32, confirm: bool) -> Result<()> {
    if !confirm {
        println!("{}", "危险操作：删除跟踪配置需要 --confirm 参数".yellow());
        return Ok(());
    }

    println!("正在删除跟踪配置: {}", id);

    match api_client
        .delete_no_content(&format!("/tracking/{}", id))
        .await
    {
        Ok(_) => {
            println!("{} 跟踪配置删除成功", "✓".green().bold());
            Ok(())
        }
        Err(e) => {
            println!("{} 删除跟踪配置失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::client::ClientConfig;
    use mockito::Server;

    async fn setup_test_server() -> (mockito::ServerGuard, ApiClient) {
        let server = Server::new_async().await;
        let config = ClientConfig {
            server_url: server.url(),
            auth_token: Some("test_token".to_string()),
            timeout: 30,
            verify_ssl: true,
        };
        let client = ApiClient::new(config).unwrap();
        (server, client)
    }

    #[test]
    fn test_parse_owner_repo_slash() {
        let result = parse_owner_repo("owner/repo");
        assert!(result.is_ok());
        let (owner, repo) = result.unwrap();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
    }

    #[test]
    fn test_parse_owner_repo_colon() {
        let result = parse_owner_repo("owner:repo");
        assert!(result.is_ok());
        let (owner, repo) = result.unwrap();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
    }

    #[test]
    fn test_parse_owner_repo_url() {
        let result = parse_owner_repo("https://gitee.com/src-openeuler/elfutils.git");
        assert!(result.is_ok());
        let (owner, repo) = result.unwrap();
        assert_eq!(owner, "src-openeuler");
        assert_eq!(repo, "elfutils");
    }

    #[test]
    fn test_parse_owner_repo_invalid() {
        let result = parse_owner_repo("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_distro_id() {
        assert_eq!(parse_distro_id("123").unwrap(), 123);
        assert!(parse_distro_id("abc").is_err());
    }

    #[tokio::test]
    async fn test_resolve_package_id_numeric() {
        let (mut _server, client) = setup_test_server().await;
        let result = resolve_package_id(&client, "42").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_resolve_package_id_by_name() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/packages")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!([
                    {
                        "id": 1,
                        "name": "test-package",
                        "level": 1,
                        "sync_interval_hours": 24,
                        "l0_repo_url": "https://example.com/repo",
                        "description": "Test package",
                        "created_at": "2024-01-01T00:00:00Z",
                        "updated_at": "2024-01-01T00:00:00Z"
                    }
                ])
                .to_string(),
            )
            .create_async()
            .await;

        let result = resolve_package_id(&client, "test-package").await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        assert_eq!(result.unwrap(), 1);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_show_tracking() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/tracking/10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "data": {
                        "id": 10,
                        "package_id": 1,
                        "distro_id": 2,
                        "l1_repo_owner": "owner",
                        "l1_repo_name": "repo",
                        "l1_branch": "main",
                        "l2_branch": "openEuler-24.03-LTS",
                        "l2_repo_path": "packages/repo",
                        "tracking_status": "active",
                        "last_sync_time": null,
                        "last_l1_commit_sha": null,
                        "last_l2_commit_sha": null,
                        "created_at": "2024-01-01T00:00:00Z",
                        "updated_at": "2024-01-01T00:00:00Z"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = show_tracking(&client, 10).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_tracking() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/tracking?page=1&page_size=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "data": {
                        "items": [
                            {
                                "id": 1,
                                "package_id": 10,
                                "distro_id": 20,
                                "l1_repo_owner": "owner1",
                                "l1_repo_name": "repo1",
                                "l1_branch": "main",
                                "l2_branch": "openEuler-24.03-LTS",
                                "l2_repo_path": "packages/repo1",
                                "tracking_status": "active",
                                "last_sync_time": null,
                                "last_l1_commit_sha": null,
                                "last_l2_commit_sha": null,
                                "created_at": "2024-01-01T00:00:00Z",
                                "updated_at": "2024-01-01T00:00:00Z"
                            }
                        ],
                        "total": 1
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = list_tracking(&client, 10, None, None).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_tracking_empty() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/tracking?page=1&page_size=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "data": {
                        "items": [],
                        "total": 0
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = list_tracking(&client, 10, None, None).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
