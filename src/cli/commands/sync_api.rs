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

//! Sync 命令处理器（通过 API）

use anyhow::Result;
use colored::Colorize;

use crate::cli::{client::ApiClient, parser::SyncAction};

/// 执行同步命令
pub async fn execute(api_client: &ApiClient, action: SyncAction) -> Result<()> {
    match action {
        SyncAction::Run {
            tracking_id,
            timeout: _,
            continue_on_error: _,
        } => run_sync(api_client, tracking_id).await,
        SyncAction::RunAll { concurrency: _ } => run_all_sync(api_client).await,
        SyncAction::Batch {
            ids,
            concurrency: _,
        } => batch_sync(api_client, ids).await,
        SyncAction::Wake { tracking_id } => wake_scheduler(api_client, tracking_id).await,
        SyncAction::Status => show_sync_status(api_client).await,
    }
}

/// 执行单个 tracking 的同步
async fn run_sync(api_client: &ApiClient, tracking_id: i32) -> Result<()> {
    println!("{}", "正在提交同步任务...".cyan());

    let result: serde_json::Value = api_client
        .post(
            &format!("/sync/{}/queue", tracking_id),
            &serde_json::json!({}),
        )
        .await?;

    println!("{}", "✓ 同步任务已提交".green());
    println!("任务 ID: {}", result["job_id"]);
    println!("状态: {}", result["status"]);

    Ok(())
}

/// 执行所有待处理的同步任务
async fn run_all_sync(api_client: &ApiClient) -> Result<()> {
    println!("{}", "正在获取所有待同步的 tracking...".cyan());

    // 获取所有 active 状态的 tracking
    let result: serde_json::Value = api_client.get("/tracking?status=active").await?;
    let trackings = result["data"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("无效的响应格式"))?;

    if trackings.is_empty() {
        println!("{}", "没有待同步的 tracking".yellow());
        return Ok(());
    }

    println!("找到 {} 个待同步的 tracking", trackings.len());

    let mut success_count = 0;
    let mut failed_count = 0;

    for tracking in trackings {
        let tracking_id = tracking["id"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("无效的 tracking ID"))? as i32;
        let package_name = tracking["package_name"].as_str().unwrap_or("unknown");

        print!("提交 {} (ID: {})... ", package_name, tracking_id);

        match api_client
            .post::<_, serde_json::Value>(
                &format!("/sync/{}/queue", tracking_id),
                &serde_json::json!({}),
            )
            .await
        {
            Ok(_) => {
                println!("{}", "✓".green());
                success_count += 1;
            }
            Err(e) => {
                println!("{}: {}", "✗".red(), e);
                failed_count += 1;
            }
        }
    }

    println!();
    println!("完成: {} 成功, {} 失败", success_count, failed_count);

    Ok(())
}

/// 批量执行指定的 tracking
async fn batch_sync(api_client: &ApiClient, ids: Vec<i32>) -> Result<()> {
    println!(
        "{}",
        format!("正在批量提交 {} 个同步任务...", ids.len()).cyan()
    );

    let mut success_count = 0;
    let mut failed_count = 0;

    for tracking_id in ids {
        print!("提交 tracking {}... ", tracking_id);

        match api_client
            .post::<_, serde_json::Value>(
                &format!("/sync/{}/queue", tracking_id),
                &serde_json::json!({}),
            )
            .await
        {
            Ok(_) => {
                println!("{}", "✓".green());
                success_count += 1;
            }
            Err(e) => {
                println!("{}: {}", "✗".red(), e);
                failed_count += 1;
            }
        }
    }

    println!();
    println!("完成: {} 成功, {} 失败", success_count, failed_count);

    Ok(())
}

/// 唤醒调度器，立即触发调度
async fn wake_scheduler(api_client: &ApiClient, tracking_id: Option<i32>) -> Result<()> {
    println!("{}", "正在唤醒调度器...".cyan());

    let body = if let Some(id) = tracking_id {
        serde_json::json!({ "tracking_id": id })
    } else {
        serde_json::json!({})
    };

    let result: serde_json::Value = api_client.post("/scheduler/wake", &body).await?;

    println!("{}", "✓ 调度器已唤醒".green());

    if let Some(message) = result["message"].as_str() {
        println!("{}", message);
    }

    Ok(())
}

/// 显示同步状态
async fn show_sync_status(api_client: &ApiClient) -> Result<()> {
    println!("{}", "正在获取同步状态...".cyan());

    // 获取调度器状态
    let status: serde_json::Value = api_client.get("/status").await?;

    println!("\n{}", "=== 系统状态 ===".bold());
    println!("数据库: {}", status["database"]["status"]);
    println!("调度器: {}", status["scheduler"]["status"]);

    if let Some(jobs) = status["scheduler"]["active_jobs"].as_array() {
        println!("\n{}", "=== 活动任务 ===".bold());
        if jobs.is_empty() {
            println!("无活动任务");
        } else {
            for job in jobs {
                println!(
                    "- 任务 {}: {} ({})",
                    job["id"], job["tracking_id"], job["status"]
                );
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
    async fn test_run_sync() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/sync/123/queue")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "job_id": 456,
                    "status": "queued"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = run_sync(&client, 123).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_run_all_sync_with_trackings() {
        let (mut server, client) = setup_test_server().await;

        let mock_list = server
            .mock("GET", "/api/tracking?status=active")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "data": [
                        {
                            "id": 10,
                            "package_name": "pkg1"
                        },
                        {
                            "id": 20,
                            "package_name": "pkg2"
                        }
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mock_sync1 = server
            .mock("POST", "/api/sync/10/queue")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "job_id": 100,
                    "status": "queued"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mock_sync2 = server
            .mock("POST", "/api/sync/20/queue")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "job_id": 200,
                    "status": "queued"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = run_all_sync(&client).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock_list.assert_async().await;
        mock_sync1.assert_async().await;
        mock_sync2.assert_async().await;
    }

    #[tokio::test]
    async fn test_run_all_sync_empty() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/tracking?status=active")
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

        let result = run_all_sync(&client).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_run_all_sync_with_failure() {
        let (mut server, client) = setup_test_server().await;

        let mock_list = server
            .mock("GET", "/api/tracking?status=active")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "data": [
                        {
                            "id": 5,
                            "package_name": "pkg-fail"
                        }
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mock_sync = server
            .mock("POST", "/api/sync/5/queue")
            .with_status(500)
            .create_async()
            .await;

        let result = run_all_sync(&client).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock_list.assert_async().await;
        mock_sync.assert_async().await;
    }

    #[tokio::test]
    async fn test_batch_sync() {
        let (mut server, client) = setup_test_server().await;

        let mock1 = server
            .mock("POST", "/api/sync/11/queue")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "job_id": 110,
                    "status": "queued"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mock2 = server
            .mock("POST", "/api/sync/22/queue")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "job_id": 220,
                    "status": "queued"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = batch_sync(&client, vec![11, 22]).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock1.assert_async().await;
        mock2.assert_async().await;
    }

    #[tokio::test]
    async fn test_batch_sync_with_failures() {
        let (mut server, client) = setup_test_server().await;

        let mock1 = server
            .mock("POST", "/api/sync/1/queue")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "job_id": 10,
                    "status": "queued"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mock2 = server
            .mock("POST", "/api/sync/2/queue")
            .with_status(500)
            .create_async()
            .await;

        let result = batch_sync(&client, vec![1, 2]).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock1.assert_async().await;
        mock2.assert_async().await;
    }

    #[tokio::test]
    async fn test_show_sync_status() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/status")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "database": {
                        "status": "connected"
                    },
                    "scheduler": {
                        "status": "running",
                        "active_jobs": []
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = show_sync_status(&client).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
