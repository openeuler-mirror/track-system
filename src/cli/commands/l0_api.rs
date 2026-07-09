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

//! L0 命令处理器（通过 API）

use anyhow::Result;
use colored::Colorize;

use crate::cli::{client::ApiClient, parser::L0Action};

/// 执行 L0 命令
pub async fn execute(api_client: &ApiClient, action: L0Action) -> Result<()> {
    match action {
        L0Action::Poll { package_id } => poll_l0(api_client, package_id).await,
        L0Action::DetectDiff { package_id } => detect_diff(api_client, package_id).await,
    }
}

/// 轮询 L0 仓库
async fn poll_l0(api_client: &ApiClient, package_id: Option<i32>) -> Result<()> {
    if let Some(id) = package_id {
        println!("{}", format!("正在轮询 package {}...", id).cyan());

        let result: serde_json::Value = api_client
            .post(&format!("/l0/poll/{}", id), &serde_json::json!({}))
            .await?;

        println!("{}", "✓ 轮询完成".green());
        println!("新 commits: {}", result["new_commits"]);
        println!("新 tags: {}", result["new_tags"]);
        println!("新 releases: {}", result["new_releases"]);
    } else {
        println!("{}", "正在轮询所有 packages...".cyan());

        let result: serde_json::Value = api_client
            .post("/l0/poll/all", &serde_json::json!({}))
            .await?;

        println!("{}", "✓ 轮询完成".green());
        println!("处理的 packages: {}", result["processed"]);
        println!("总新 commits: {}", result["total_new_commits"]);
        println!("总新 tags: {}", result["total_new_tags"]);
        println!("总新 releases: {}", result["total_new_releases"]);
    }

    Ok(())
}

/// 检测 L0 与 L1 的差异
async fn detect_diff(api_client: &ApiClient, package_id: i32) -> Result<()> {
    println!(
        "{}",
        format!("正在检测 package {} 的 L0/L1 差异...", package_id).cyan()
    );

    let result: serde_json::Value = api_client
        .post(&format!("/l0/diff/{}", package_id), &serde_json::json!({}))
        .await?;

    println!("{}", "✓ 差异检测完成".green());
    println!("\n{}", "=== 版本信息 ===".bold());
    println!("L0 最新版本: {}", result["l0_version"]);
    println!("L1 当前版本: {}", result["l1_version"]);

    if let Some(diff) = result["diff"].as_object() {
        println!("\n{}", "=== 差异统计 ===".bold());
        println!("新增 commits: {}", diff["new_commits"]);
        println!("新增 tags: {}", diff["new_tags"]);
        println!("版本落后: {}", diff["version_behind"]);

        if let Some(upgrade_available) = diff["upgrade_available"].as_bool() {
            if upgrade_available {
                println!("\n{}", "有可用的升级版本".yellow());
            }
        }
    }

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
    async fn test_poll_l0_single_package() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/l0/poll/123")
            .match_body(mockito::Matcher::Json(serde_json::json!({})))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "new_commits": 10,
                    "new_tags": 2,
                    "new_releases": 1
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = poll_l0(&client, Some(123)).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_poll_l0_all_packages() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/l0/poll/all")
            .match_body(mockito::Matcher::Json(serde_json::json!({})))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "processed": 5,
                    "total_new_commits": 50,
                    "total_new_tags": 10,
                    "total_new_releases": 3
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = poll_l0(&client, None).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_detect_diff() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/l0/diff/456")
            .match_body(mockito::Matcher::Json(serde_json::json!({})))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "l0_version": "2.0.0",
                    "l1_version": "1.5.0",
                    "diff": {
                        "new_commits": 25,
                        "new_tags": 3,
                        "version_behind": 1,
                        "upgrade_available": true
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = detect_diff(&client, 456).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_detect_diff_no_upgrade() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/l0/diff/789")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "l0_version": "1.0.0",
                    "l1_version": "1.0.0",
                    "diff": {
                        "new_commits": 0,
                        "new_tags": 0,
                        "version_behind": 0,
                        "upgrade_available": false
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = detect_diff(&client, 789).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_execute_poll_action() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/l0/poll/100")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
