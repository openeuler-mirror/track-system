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

//! Snapshot 命令处理器（通过 API）

use anyhow::Result;
use colored::Colorize;

use crate::cli::formatter::format_datetime_local;
use crate::cli::{client::ApiClient, parser::SnapshotAction};

/// 执行快照命令
pub async fn execute(api_client: &ApiClient, action: SnapshotAction) -> Result<()> {
    match action {
        SnapshotAction::Create { tracking_id, tag } => {
            create_snapshot(api_client, tracking_id, tag).await
        }
        SnapshotAction::Restore { snapshot_id, force } => {
            restore_snapshot(api_client, snapshot_id, force).await
        }
        SnapshotAction::List { tracking_id } => list_snapshots(api_client, tracking_id).await,
        SnapshotAction::Delete { snapshot_id } => delete_snapshot(api_client, snapshot_id).await,
    }
}

/// 创建 L2 快照
async fn create_snapshot(
    api_client: &ApiClient,
    tracking_id: i32,
    tag: Option<String>,
) -> Result<()> {
    println!(
        "{}",
        format!("正在创建 tracking {} 的快照...", tracking_id).cyan()
    );

    let mut payload = serde_json::json!({
        "tracking_id": tracking_id
    });

    if let Some(t) = tag {
        payload["tag"] = serde_json::Value::String(t);
    }

    let result: serde_json::Value = api_client.post("/snapshot/create", &payload).await?;

    println!("{}", "✓ 快照已创建".green());
    println!("快照 ID: {}", result["snapshot_id"]);
    let created_at = result["created_at"].as_str().unwrap_or("-");
    let created_at = chrono::DateTime::parse_from_rfc3339(created_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .ok()
        .map(|dt| format_datetime_local(&dt))
        .unwrap_or_else(|| created_at.to_string());
    println!("创建时间: {}", created_at);
    if let Some(tag_value) = result["tag"].as_str() {
        println!("标签: {}", tag_value);
    }

    Ok(())
}

/// 恢复 L2 快照
async fn restore_snapshot(api_client: &ApiClient, snapshot_id: i64, force: bool) -> Result<()> {
    println!("{}", format!("正在恢复快照 {}...", snapshot_id).cyan());

    if !force {
        println!("{}", "警告: 此操作将覆盖现有数据".yellow());
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

    let result: serde_json::Value = api_client
        .post(
            &format!("/snapshot/{}/restore", snapshot_id),
            &serde_json::json!({
                "force": force
            }),
        )
        .await?;

    println!("{}", "✓ 快照已恢复".green());
    println!("恢复的记录数: {}", result["restored_records"]);
    let restored_at = result["restored_at"].as_str().unwrap_or("-");
    let restored_at = chrono::DateTime::parse_from_rfc3339(restored_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .ok()
        .map(|dt| format_datetime_local(&dt))
        .unwrap_or_else(|| restored_at.to_string());
    println!("恢复时间: {}", restored_at);

    Ok(())
}

/// 列出快照
async fn list_snapshots(api_client: &ApiClient, tracking_id: Option<i32>) -> Result<()> {
    let url = if let Some(id) = tracking_id {
        format!("/snapshot/list?tracking_id={}", id)
    } else {
        "/snapshot/list".to_string()
    };

    println!("{}", "正在获取快照列表...".cyan());

    let result: serde_json::Value = api_client.get(&url).await?;
    let snapshots = result["snapshots"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("无效的响应格式"))?;

    if snapshots.is_empty() {
        println!("{}", "没有快照".yellow());
        return Ok(());
    }

    println!("\n{}", "=== 快照列表 ===".bold());
    println!(
        "{:<10} {:<15} {:<20} {:<30}",
        "ID", "Tracking ID", "标签", "创建时间"
    );
    println!("{}", "-".repeat(75));

    for snapshot in snapshots {
        let id = snapshot["id"].as_i64().unwrap_or(0);
        let tracking_id = snapshot["tracking_id"].as_i64().unwrap_or(0);
        let tag = snapshot["tag"].as_str().unwrap_or("-");
        let created_at_raw = snapshot["created_at"].as_str().unwrap_or("-");
        let created_at = chrono::DateTime::parse_from_rfc3339(created_at_raw)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .ok()
            .map(|dt| format_datetime_local(&dt))
            .unwrap_or_else(|| created_at_raw.to_string());

        println!(
            "{:<10} {:<15} {:<20} {:<30}",
            id, tracking_id, tag, created_at
        );
    }

    Ok(())
}

/// 删除快照
async fn delete_snapshot(api_client: &ApiClient, snapshot_id: i32) -> Result<()> {
    println!("{}", format!("正在删除快照 {}...", snapshot_id).cyan());

    match api_client
        .delete_no_content(&format!("/snapshot/{}", snapshot_id))
        .await
    {
        Ok(_) => {
            println!("{}", "✓ 快照已删除".green());
            Ok(())
        }
        Err(e) => {
            println!("{} 删除快照失败: {}", "✗".red().bold(), e);
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
    async fn test_create_snapshot_without_tag() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/snapshot/create")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "tracking_id": 1
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "snapshot_id": "snap-123",
                    "created_at": "2024-01-01T00:00:00Z"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = create_snapshot(&client, 1, None).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_snapshot_with_tag() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/snapshot/create")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "tracking_id": 2,
                "tag": "v1.0.0"
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "snapshot_id": "snap-456",
                    "created_at": "2024-01-01T00:00:00Z",
                    "tag": "v1.0.0"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = create_snapshot(&client, 2, Some("v1.0.0".to_string())).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_restore_snapshot_with_force() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("POST", "/api/snapshot/123/restore")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "force": true
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "restored_records": 1000,
                    "restored_at": "2024-01-01T01:00:00Z"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = restore_snapshot(&client, 123, true).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_snapshots_all() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/snapshot/list")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "snapshots": [
                        {
                            "id": 1,
                            "tracking_id": 10,
                            "tag": "v1.0",
                            "created_at": "2024-01-01T00:00:00Z"
                        },
                        {
                            "id": 2,
                            "tracking_id": 20,
                            "tag": "v2.0",
                            "created_at": "2024-01-02T00:00:00Z"
                        }
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = list_snapshots(&client, None).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_snapshots_by_tracking() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/snapshot/list?tracking_id=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "snapshots": [
                        {
                            "id": 1,
                            "tracking_id": 10,
