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
    sync_interval: String,
    l0_repo: Option<String>,
    description: Option<String>,
) -> Result<()> {
    println!("正在添加软件包: {}", name.cyan());

    let sync_interval_hours = parse_sync_interval_hours(&sync_interval)?;
    let request = CreatePackageRequest {
        name: name.clone(),
        level,
        sync_interval_hours,
        l0_repo_url: l0_repo,
        description,
    };

    // 服务端返回裸 PackageResponse
    match api_client
        .post::<_, PackageDto>("/packages", &request)
        .await
    {
        Ok(pkg) => {
            println!("{} 软件包添加成功", "✓".green().bold());
            println!("  ID: {}", pkg.id);
            println!("  名称: {}", pkg.name.cyan());
            println!("  等级: {}", pkg.level);
            println!("  同步间隔: {} 小时", pkg.sync_interval_hours);
            if let Some(url) = pkg.l0_repo_url.clone() {
                println!("  L0 仓库: {}", url);
            }
            if let Some(desc) = pkg.description.clone() {
                println!("  描述: {}", desc);
            }
            Ok(())
        }
        Err(e) => {
            println!("{} 添加软件包失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 列出软件包
async fn list_packages(api_client: &ApiClient, limit: u64) -> Result<()> {
    println!("正在获取软件包列表...");

    // 服务端 /packages 返回 Vec<PackageResponse>
    match api_client.get::<Vec<PackageDto>>("/packages").await {
        Ok(packages) => {
            let packages = if limit > 0 {
                packages
                    .into_iter()
                    .take(limit as usize)
                    .collect::<Vec<_>>()
            } else {
                packages
            };

            if packages.is_empty() {
                println!("{}", "没有找到软件包".yellow());
                return Ok(());
            }

            println!();
            println!("{}", "软件包列表:".bold());
            println!(
                "{:<5} {:<20} {:<6} {:<8} {:<50}",
                "ID", "名称", "等级", "间隔", "描述"
            );
            println!("{}", "-".repeat(95));

            for pkg in packages {
                println!(
                    "{:<5} {:<20} {:<6} {:<8} {:<50}",
                    pkg.id,
                    pkg.name.cyan(),
                    pkg.level,
                    pkg.sync_interval_hours,
                    pkg.description.clone().unwrap_or_else(|| "N/A".to_string())
                );
            }

            Ok(())
        }
        Err(e) => {
            println!("{} 获取软件包列表失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 显示软件包详情
async fn show_package(api_client: &ApiClient, name_or_id: String) -> Result<()> {
    println!("正在获取软件包详情: {}", name_or_id.cyan());

    // 优先按 ID 查询，否则按名称客户端过滤
    if let Ok(id) = name_or_id.parse::<i32>() {
        match api_client
            .get::<PackageDto>(&format!("/packages/{}", id))
            .await
        {
            Ok(pkg) => {
                print_package_detail(&pkg);
                Ok(())
            }
            Err(e) => {
                println!("{} 获取软件包详情失败: {}", "✗".red().bold(), e);
                Err(e.into())
            }
        }
    } else {
        match find_package_by_name(api_client, &name_or_id).await? {
            Some(pkg) => {
                print_package_detail(&pkg);
                Ok(())
            }
            None => {
                println!("{} 未找到软件包: {}", "✗".red().bold(), name_or_id);
                Ok(())
            }
        }
    }
}

fn print_package_detail(pkg: &PackageDto) {
    println!();
    println!("{}", "软件包详情:".bold());
    println!("  ID: {}", pkg.id);
    println!("  名称: {}", pkg.name.cyan());
    println!("  等级: {}", pkg.level);
    println!("  同步间隔: {} 小时", pkg.sync_interval_hours);
    if let Some(url) = pkg.l0_repo_url.clone() {
        println!("  L0 仓库: {}", url);
    }
    if let Some(desc) = pkg.description.clone() {
        println!("  描述: {}", desc);
    }
    println!("  创建时间: {}", format_datetime_local(&pkg.created_at));
    println!("  更新时间: {}", format_datetime_local(&pkg.updated_at));
}

/// 更新软件包
async fn update_package(
    api_client: &ApiClient,
    name: String,
    sync_interval: Option<String>,
    level: Option<i32>,
    description: Option<String>,
) -> Result<()> {
    println!("正在更新软件包: {}", name.cyan());

    let pkg_opt = find_package_by_name(api_client, &name).await?;
    let pkg = match pkg_opt {
        Some(p) => p,
        None => {
            println!("{} 未找到软件包: {}", "✗".red().bold(), name);
            return Ok(());
        }
    };

    let request = UpdatePackageRequest {
        level,
        sync_interval_hours: match sync_interval {
            Some(s) => Some(parse_sync_interval_hours(&s)?),
            None => None,
        },
        l0_repo_url: None,
        description,
    };

    match api_client
        .put::<_, PackageDto>(&format!("/packages/{}", pkg.id), &request)
        .await
    {
        Ok(_) => {
            println!("{} 软件包更新成功", "✓".green().bold());
            Ok(())
        }
        Err(e) => {
            println!("{} 更新软件包失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 删除软件包
async fn remove_package(api_client: &ApiClient, name: String, confirm: bool) -> Result<()> {
    if !confirm {
        println!("{}", "危险操作：删除软件包需要 --confirm 参数".yellow());
        return Ok(());
    }

    println!("正在删除软件包: {}", name.cyan());

    let pkg_opt = find_package_by_name(api_client, &name).await?;
    let pkg = match pkg_opt {
        Some(p) => p,
        None => bail!("未找到软件包: {}", name),
    };

    match api_client
        .delete_no_content(&format!("/packages/{}", pkg.id))
        .await
    {
        Ok(_) => {
            println!("{} 软件包删除成功", "✓".green().bold());
            Ok(())
        }
        Err(e) => {
            println!("{} 删除软件包失败: {}", "✗".red().bold(), e);
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

    fn create_test_package_dto(id: i32, name: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "name": name,
            "level": 1,
            "sync_interval_hours": 24,
            "l0_repo_url": "https://github.com/test/repo",
            "description": "Test package",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        })
    }

    #[test]
    fn test_parse_sync_interval_hours_ok() {
        assert_eq!(parse_sync_interval_hours("12h").unwrap(), 12);
        assert_eq!(parse_sync_interval_hours("24").unwrap(), 24);
        assert_eq!(parse_sync_interval_hours("\"6h\"").unwrap(), 6);
        assert_eq!(parse_sync_interval_hours("'48h'").unwrap(), 48);
        assert_eq!(parse_sync_interval_hours("8760H").unwrap(), 8760);
    }

    #[test]
    fn test_parse_sync_interval_hours_invalid() {
        assert!(parse_sync_interval_hours("").is_err());
        assert!(parse_sync_interval_hours("0h").is_err());
        assert!(parse_sync_interval_hours("-1h").is_err());
        assert!(parse_sync_interval_hours("abc").is_err());
        assert!(parse_sync_interval_hours("12m").is_err());
        assert!(parse_sync_interval_hours("8761h").is_err());
    }

    #[tokio::test]
    async fn test_add_package() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/packages")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "name": "test-package",
                "level": 1,
                "sync_interval_hours": 24,
                "l0_repo_url": "https://github.com/test/repo",
                "description": "Test package"
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(create_test_package_dto(1, "test-package").to_string())
            .create_async()
            .await;

        let result = add_package(
            &client,
            "test-package".to_string(),
            1,
            "24h".to_string(),
            Some("https://github.com/test/repo".to_string()),
            Some("Test package".to_string()),
        )
        .await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_packages() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/packages")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!([
                    create_test_package_dto(1, "package1"),
                    create_test_package_dto(2, "package2")
                ])
                .to_string(),
            )
            .create_async()
            .await;
