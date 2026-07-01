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

//! Classify 命令处理器（通过 API）

use anyhow::Result;
use colored::Colorize;

use crate::cli::{client::ApiClient, parser::ClassifyAction};

/// 执行分类命令
pub async fn execute(api_client: &ApiClient, action: ClassifyAction) -> Result<()> {
    match action {
        ClassifyAction::Process { limit } => process_classification(api_client, limit).await,
        ClassifyAction::ProcessTracking { tracking_id, limit } => {
            process_tracking_classification(api_client, tracking_id, limit).await
        }
        ClassifyAction::Daemon {
            interval,
            batch_size,
        } => run_classification_daemon(api_client, interval, batch_size).await,
    }
}

/// 处理待分类的 commits
async fn process_classification(api_client: &ApiClient, limit: usize) -> Result<()> {
    println!(
        "{}",
        format!("正在处理待分类的 commits (限制: {})...", limit).cyan()
    );

    let result: serde_json::Value = api_client
        .post(
            "/classify/process",
            &serde_json::json!({
                "limit": limit
            }),
        )
        .await?;

    println!("{}", "✓ 分类任务已完成".green());
    println!("处理数量: {}", result["processed"]);
    println!("成功: {}", result["success"]);
    println!("失败: {}", result["failed"]);

    Ok(())
}

/// 处理指定 tracking 的分类任务
async fn process_tracking_classification(
    api_client: &ApiClient,
    tracking_id: i32,
    limit: usize,
) -> Result<()> {
    println!(
        "{}",
        format!(
            "正在处理 tracking {} 的分类任务 (限制: {})...",
            tracking_id, limit
        )
        .cyan()
    );

    let result: serde_json::Value = api_client
        .post(
            &format!("/classify/tracking/{}", tracking_id),
            &serde_json::json!({
                "limit": limit
            }),
        )
        .await?;

    println!("{}", "✓ 分类任务已完成".green());
    println!("处理数量: {}", result["processed"]);
    println!("成功: {}", result["success"]);
    println!("失败: {}", result["failed"]);

    Ok(())
}

/// 以守护进程方式运行分类任务队列
async fn run_classification_daemon(
    api_client: &ApiClient,
    interval: u64,
    batch_size: usize,
) -> Result<()> {
    println!(
        "{}",
        format!(
            "启动分类守护进程 (间隔: {}秒, 批大小: {})...",
            interval, batch_size
        )
        .cyan()
    );

    let result: serde_json::Value = api_client
        .post(
            "/classify/daemon/start",
            &serde_json::json!({
                "interval": interval,
                "batch_size": batch_size
            }),
        )
        .await?;

    println!("{}", "✓ 守护进程已启动".green());
    println!("守护进程 ID: {}", result["daemon_id"]);
    println!("状态: {}", result["status"]);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::client::ClientConfig;
    use mockito::{Server, ServerGuard};

    async fn setup_test_server() -> (ServerGuard, ApiClient) {
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
    async fn test_process_classification() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/classify/process")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "limit": 10
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "processed": 10,
                    "success": 8,
                    "failed": 2
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = process_classification(&client, 10).await;
        if let Err(e) = &result {
            eprintln!("Test error: {:?}", e);
        }
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_process_tracking_classification() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/classify/tracking/123")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "limit": 20
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "processed": 20,
                    "success": 18,
                    "failed": 2
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = process_tracking_classification(&client, 123, 20).await;
        if let Err(e) = &result {
            eprintln!("Test error: {:?}", e);
        }
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_run_classification_daemon() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/classify/daemon/start")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "interval": 60,
                "batch_size": 100
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "daemon_id": "daemon-123",
                    "status": "running"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = run_classification_daemon(&client, 60, 100).await;
        if let Err(e) = &result {
            eprintln!("Test error: {:?}", e);
        }
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_execute_process_action() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/classify/process")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "processed": 5,
                    "success": 5,
                    "failed": 0
                })
                .to_string(),
            )
            .create_async()
            .await;

        let action = ClassifyAction::Process { limit: 5 };
        let result = execute(&client, action).await;
        if let Err(e) = &result {
            eprintln!("Test error: {:?}", e);
        }
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_execute_process_tracking_action() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/classify/tracking/456")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "processed": 15,
                    "success": 14,
                    "failed": 1
                })
                .to_string(),
            )
            .create_async()
            .await;

        let action = ClassifyAction::ProcessTracking {
            tracking_id: 456,
