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

//! Distro 命令处理器（通过 API）

use anyhow::Result;
use colored::Colorize;

use crate::cli::{client::ApiClient, parser::DistroAction};

/// 执行发行版管理命令
pub async fn execute(api_client: &ApiClient, action: DistroAction) -> Result<()> {
    match action {
        DistroAction::Add {
            name,
            version,
            description,
        } => add_distro(api_client, name, version, description).await,
        DistroAction::List => list_distros(api_client).await,
        DistroAction::Show { name_or_id } => show_distro(api_client, name_or_id).await,
        DistroAction::Remove { name, confirm } => remove_distro(api_client, name, confirm).await,
    }
}

/// 添加发行版
async fn add_distro(
    api_client: &ApiClient,
    name: String,
    version: String,
    description: Option<String>,
) -> Result<()> {
    println!(
        "{}",
        format!("正在添加发行版 {}:{}...", name, version).cyan()
    );

    let mut payload = serde_json::json!({
        "name": name,
        "version": version
    });

    if let Some(desc) = description {
        payload["description"] = serde_json::Value::String(desc);
    }

    let result: serde_json::Value = api_client.post("/distros", &payload).await?;

    println!("{}", "✓ 发行版已添加".green());
    println!("ID: {}", result["id"]);
    println!("名称: {}", result["name"]);
    println!("版本: {}", result["version"]);

    Ok(())
}

/// 列出所有发行版
async fn list_distros(api_client: &ApiClient) -> Result<()> {
    println!("{}", "正在获取发行版列表...".cyan());

    let result: serde_json::Value = api_client.get("/distros").await?;
    let distros = result["data"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("无效的响应格式"))?;

    if distros.is_empty() {
        println!("{}", "没有发行版".yellow());
        return Ok(());
    }

    println!("\n{}", "=== 发行版列表 ===".bold());
    println!("{:<10} {:<20} {:<15} {:<30}", "ID", "名称", "版本", "描述");
    println!("{}", "-".repeat(75));

    for distro in distros {
        let id = distro["id"].as_i64().unwrap_or(0);
        let name = distro["name"].as_str().unwrap_or("-");
        let version = distro["version"].as_str().unwrap_or("-");
        let description = distro["description"].as_str().unwrap_or("-");

        println!(
            "{:<10} {:<20} {:<15} {:<30}",
            id, name, version, description
        );
    }

    Ok(())
}

/// 显示发行版详情
async fn show_distro(api_client: &ApiClient, name_or_id: String) -> Result<()> {
    println!(
        "{}",
        format!("正在获取发行版 {} 的详情...", name_or_id).cyan()
    );

    // 尝试作为 ID 解析
    let url = if let Ok(id) = name_or_id.parse::<i32>() {
        format!("/distros/{}", id)
    } else {
        format!("/distros/by-name/{}", name_or_id)
    };

    let distro: serde_json::Value = api_client.get(&url).await?;

    println!("\n{}", "=== 发行版详情 ===".bold());
    println!("ID: {}", distro["id"]);
    println!("名称: {}", distro["name"]);
    println!("版本: {}", distro["version"]);
    println!("描述: {}", distro["description"].as_str().unwrap_or("-"));
    println!("创建时间: {}", distro["created_at"]);
    println!("更新时间: {}", distro["updated_at"]);

    // 显示关联的 tracking 数量
    if let Some(tracking_count) = distro["tracking_count"].as_i64() {
        println!("\n关联的 tracking 数量: {}", tracking_count);
    }

    Ok(())
}

/// 删除发行版
async fn remove_distro(api_client: &ApiClient, name: String, confirm: bool) -> Result<()> {
    if !confirm {
        println!("{}", "警告: 此操作将删除发行版及其所有关联数据".yellow());
        print!("是否继续? (y/N): ");
        use std::io::{self, Write};
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("已取消");
            return Ok(());
        }
    }

    println!("{}", format!("正在删除发行版 {}...", name).cyan());

    let _result: serde_json::Value = api_client
        .delete(&format!("/distros/by-name/{}", name))
        .await?;

    println!("{}", "✓ 发行版已删除".green());

    Ok(())
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
    async fn test_add_distro_without_description() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/distros")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "name": "Ubuntu",
                "version": "22.04"
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "id": 1,
                    "name": "Ubuntu",
                    "version": "22.04"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = add_distro(&client, "Ubuntu".to_string(), "22.04".to_string(), None).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_add_distro_with_description() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/distros")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "name": "Debian",
                "version": "12",
                "description": "Debian 12 Bookworm"
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "id": 2,
                    "name": "Debian",
                    "version": "12"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = add_distro(
            &client,
            "Debian".to_string(),
            "12".to_string(),
            Some("Debian 12 Bookworm".to_string()),
        )
        .await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_distros() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/distros")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "data": [
                        {
                            "id": 1,
                            "name": "Ubuntu",
                            "version": "22.04",
                            "description": "Ubuntu LTS"
                        },
                        {
                            "id": 2,
                            "name": "Debian",
                            "version": "12",
                            "description": "Debian Stable"
                        }
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = list_distros(&client).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_distros_empty() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/distros")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "data": []
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = list_distros(&client).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_show_distro_by_id() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/distros/123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "id": 123,
                    "name": "Fedora",
                    "version": "39",
                    "description": "Fedora 39",
                    "created_at": "2024-01-01T00:00:00Z",
                    "updated_at": "2024-01-01T00:00:00Z",
                    "tracking_count": 5
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = show_distro(&client, "123".to_string()).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_show_distro_by_name() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/distros/by-name/CentOS")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "id": 456,
                    "name": "CentOS",
