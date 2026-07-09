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

//! 服务器管理命令实现
//!
//! 提供服务器连接配置、测试和信息查询功能

use anyhow::Result;
use colored::Colorize;

use crate::cli::client::{ApiClient, ClientConfig};
use crate::cli::parser::ServerAction;

/// 执行服务器管理命令
pub async fn execute(api_client: &ApiClient, action: ServerAction) -> Result<()> {
    match action {
        ServerAction::Config { url, token, show } => {
            execute_config(api_client, url, token, show).await
        }
        ServerAction::Ping => execute_ping(api_client).await,
        ServerAction::Info => execute_info(api_client).await,
    }
}

/// 配置服务器连接
async fn execute_config(
    _api_client: &ApiClient,
    url: Option<String>,
    token: Option<String>,
    show: bool,
) -> Result<()> {
    if show {
        // 显示当前配置
        let config = ClientConfig::from_env()?;
        println!("{}", "当前服务器配置:".bold());
        println!("  服务器地址: {}", config.server_url.cyan());
        println!(
            "  认证 Token: {}",
            config
                .auth_token
                .as_ref()
                .map(|t| format!("{}...", &t[..t.len().min(10)]))
                .unwrap_or_else(|| "未设置".to_string())
                .yellow()
        );
        println!("  超时时间: {} 秒", config.timeout);
        println!("  SSL 验证: {}", config.verify_ssl);
        println!();
        println!("配置文件路径: {}", ClientConfig::config_path()?.display());
        return Ok(());
    }

    // 加载现有配置
    let mut config = ClientConfig::from_env()?;
    let mut changed = false;

    // 更新配置
    if let Some(new_url) = url {
        config.set_server_url(new_url.clone());
        println!("{} 服务器地址: {}", "✓".green(), new_url.cyan());
        changed = true;
    }

    if let Some(new_token) = token {
        config.set_auth_token(Some(new_token.clone()));
        println!(
            "{} 认证 Token: {}...",
            "✓".green(),
            &new_token[..new_token.len().min(10)].yellow()
        );
        changed = true;
    }

    if changed {
        // 验证配置
        config.validate()?;

        // 保存配置
        config.save()?;
        println!();
        println!("{} 配置已保存", "✓".green().bold());
        println!("配置文件: {}", ClientConfig::config_path()?.display());
    } else {
        println!("{}", "未指定任何配置项".yellow());
        println!("使用 --url 设置服务器地址");
        println!("使用 --token 设置认证 token");
        println!("使用 --show 显示当前配置");
    }

    Ok(())
}

/// 测试服务器连接
async fn execute_ping(api_client: &ApiClient) -> Result<()> {
    println!("正在测试服务器连接...");
    println!("服务器: {}", api_client.config().server_url.cyan());
    println!();

    match api_client.ping().await {
        Ok(true) => {
            println!("{} 服务器连接成功", "✓".green().bold());
            Ok(())
        }
        Ok(false) => {
            println!("{} 无法连接到服务器", "✗".red().bold());
            println!();
            println!("请检查:");
            println!("  1. 服务器地址是否正确");
            println!("  2. track-server 是否正在运行");
            println!("  3. 网络连接是否正常");
            anyhow::bail!("服务器连接失败")
        }
        Err(e) => {
            println!("{} 连接测试失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 获取服务器信息
async fn execute_info(api_client: &ApiClient) -> Result<()> {
    println!("正在获取服务器信息...");
    println!();

    match api_client.health_check().await {
        Ok(health) => {
            println!("{}", "服务器信息:".bold());
            println!("  服务器地址: {}", api_client.config().server_url.cyan());
            println!();
            println!("{}", "健康状态:".bold());
            println!("{}", serde_json::to_string_pretty(&health)?);
            Ok(())
        }
        Err(e) => {
            println!("{} 获取服务器信息失败: {}", "✗".red().bold(), e);
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

    #[tokio::test]
    async fn test_execute_ping_success() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/health")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "status": "healthy"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = execute_ping(&client).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_execute_ping_failure() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/health")
            .with_status(500)
            .create_async()
            .await;

        let result = execute_ping(&client).await;
        assert!(result.is_err(), "Expected failure but got success");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_execute_info_success() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/health")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "status": "healthy",
                    "database": "connected",
                    "version": "1.0.0"
                })
                .to_string(),
            )
            .create_async()
            .await;

