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

//! Health 命令处理器（通过 API）

use anyhow::Result;
use colored::Colorize;

use crate::cli::{client::ApiClient, parser::HealthAction};

/// 执行健康检查命令
pub async fn execute(api_client: &ApiClient, action: HealthAction) -> Result<()> {
    match action {
        HealthAction::Check { component } => check_health(api_client, component).await,
    }
}

/// 检查系统健康状态
async fn check_health(api_client: &ApiClient, component: Option<String>) -> Result<()> {
    if let Some(comp) = component {
        check_component_health(api_client, &comp).await
    } else {
        check_all_health(api_client).await
    }
}

/// 检查所有组件健康状态
async fn check_all_health(api_client: &ApiClient) -> Result<()> {
    println!("{}", "正在检查系统健康状态...".cyan());

    let health: serde_json::Value = api_client.get("/health").await?;

    println!("\n{}", "=== 系统健康状态 ===".bold());

    // 整体状态
    let overall_status = health["status"].as_str().unwrap_or("unknown");
    let status_display = match overall_status {
        "healthy" => "健康".green(),
        "degraded" => "降级".yellow(),
        "unhealthy" => "不健康".red(),
        _ => "未知".white(),
    };
    println!("整体状态: {}", status_display);

    // 数据库状态
    if let Some(db) = health["database"].as_object() {
        println!("\n{}", "数据库:".bold());
        let db_status = db
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let db_display = match db_status {
            "connected" => "已连接".green(),
            "disconnected" => "未连接".red(),
            _ => "未知".white(),
        };
        println!("  状态: {}", db_display);

        if let Some(latency) = db.get("latency_ms").and_then(|v| v.as_f64()) {
            println!("  延迟: {:.2} ms", latency);
        }
    }

    // 调度器状态
    if let Some(scheduler) = health["scheduler"].as_object() {
        println!("\n{}", "调度器:".bold());
        let sched_status = scheduler
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let sched_display = match sched_status {
            "running" => "运行中".green(),
            "stopped" => "已停止".yellow(),
            _ => "未知".white(),
        };
        println!("  状态: {}", sched_display);

        if let Some(active_jobs) = scheduler.get("active_jobs").and_then(|v| v.as_i64()) {
            println!("  活动任务: {}", active_jobs);
        }

        if let Some(pending_jobs) = scheduler.get("pending_jobs").and_then(|v| v.as_i64()) {
            println!("  待处理任务: {}", pending_jobs);
        }
    }

    // API 状态
    if let Some(api) = health["api"].as_object() {
        println!("\n{}", "API:".bold());
        let api_status = api
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let api_display = match api_status {
            "available" => "可用".green(),
            "unavailable" => "不可用".red(),
            _ => "未知".white(),
        };
        println!("  状态: {}", api_display);

        if let Some(version) = api.get("version").and_then(|v| v.as_str()) {
            println!("  版本: {}", version);
        }
    }

    // 时间戳
    if let Some(timestamp) = health["timestamp"].as_str() {
        println!("\n检查时间: {}", timestamp);
    }

    Ok(())
}

/// 检查特定组件健康状态
async fn check_component_health(api_client: &ApiClient, component: &str) -> Result<()> {
    println!(
        "{}",
        format!("正在检查 {} 组件健康状态...", component).cyan()
    );

    let health: serde_json::Value = api_client
        .get(&format!("/health?component={}", component))
        .await?;

    println!("\n{}", format!("=== {} 健康状态 ===", component).bold());

    let status = health["status"].as_str().unwrap_or("unknown");
    let status_display = match status {
        "healthy" => "健康".green(),
        "degraded" => "降级".yellow(),
        "unhealthy" => "不健康".red(),
        _ => "未知".white(),
    };
    println!("状态: {}", status_display);

    // 显示详细信息
    if let Some(details) = health["details"].as_object() {
        println!("\n详细信息:");
        for (key, value) in details {
            println!("  {}: {}", key, value);
        }
    }

    // 显示错误信息
    if let Some(error) = health["error"].as_str() {
        println!("\n{}: {}", "错误".red(), error);
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
    async fn test_check_all_health() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/health")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "status": "healthy",
                    "database": {
                        "status": "connected",
                        "latency_ms": 5.2
                    },
                    "scheduler": {
                        "status": "running",
                        "active_jobs": 3,
                        "pending_jobs": 10
                    },
                    "api": {
                        "status": "available",
                        "version": "1.0.0"
                    },
                    "timestamp": "2024-01-01T00:00:00Z"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = check_all_health(&client).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_check_component_health() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/health?component=database")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "status": "healthy",
                    "details": {
                        "connection_pool": "active",
                        "connections": 10
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = check_component_health(&client, "database").await;
