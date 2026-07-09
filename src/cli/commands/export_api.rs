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

//! Export 命令处理器（通过 API）

use anyhow::Result;
use colored::Colorize;
use std::fs;

use crate::cli::{client::ApiClient, parser::ExportAction};

/// 执行导出命令
pub async fn execute(api_client: &ApiClient, action: ExportAction) -> Result<()> {
    match action {
        ExportAction::Metadata {
            format,
            output,
            package_id,
        } => export_metadata(api_client, format, output, package_id).await,
        ExportAction::Report {
            report_id,
            format,
            output,
        } => export_report(api_client, report_id, format, output).await,
    }
}

/// 导出元数据
async fn export_metadata(
    api_client: &ApiClient,
    format: String,
    output: Option<String>,
    package_id: Option<i32>,
) -> Result<()> {
    println!("{}", "正在导出元数据...".cyan());

    let mut url = format!("/export/metadata?format={}", format);
    if let Some(id) = package_id {
        url.push_str(&format!("&package_id={}", id));
    }

    let content: String = api_client.get(&url).await?;

    // 保存到文件或输出到控制台
    if let Some(output_path) = output {
        fs::write(&output_path, &content)?;
        println!("{}", "✓ 元数据已导出".green());
        println!("文件: {}", output_path);
    } else {
        println!("{}", content);
    }

    Ok(())
}

/// 导出报告
async fn export_report(
    api_client: &ApiClient,
    report_id: i32,
    format: String,
    output: Option<String>,
) -> Result<()> {
    println!(
        "{}",
        format!("正在导出报告 {} (格式: {})...", report_id, format).cyan()
    );

    let url = format!("/export/report/{}?format={}", report_id, format);

    let content: String = api_client.get(&url).await?;

    // 保存到文件或输出到控制台
    if let Some(output_path) = output {
        fs::write(&output_path, &content)?;
        println!("{}", "✓ 报告已导出".green());
        println!("文件: {}", output_path);
        println!("大小: {} bytes", content.len());
    } else {
        println!("{}", content);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::client::ClientConfig;
    use mockito::Server;
    use tempfile::NamedTempFile;

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
    async fn test_export_metadata_to_console() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/export/metadata?format=json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("\"{\\\"data\\\": \\\"test metadata\\\"}\"")
            .create_async()
            .await;

        let result = export_metadata(&client, "json".to_string(), None, None).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_export_metadata_with_package_id() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/export/metadata?format=yaml&package_id=123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("\"name: test\\nversion: 1.0\"")
            .create_async()
            .await;

        let result = export_metadata(&client, "yaml".to_string(), None, Some(123)).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_export_metadata_to_file() {
        let (mut server, client) = setup_test_server().await;
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_str().unwrap().to_string();

        let mock = server
            .mock("GET", "/api/export/metadata?format=json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("\"{\\\"test\\\": \\\"data\\\"}\"")
            .create_async()
            .await;

        let result =
            export_metadata(&client, "json".to_string(), Some(file_path.clone()), None).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "{\"test\": \"data\"}");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_export_report_to_console() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
            .mock("GET", "/api/export/report/456?format=html")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("\"<html><body>Report</body></html>\"")
            .create_async()
            .await;

        let result = export_report(&client, 456, "html".to_string(), None).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_export_report_to_file() {
        let (mut server, client) = setup_test_server().await;
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_str().unwrap().to_string();

        let mock = server
            .mock("GET", "/api/export/report/789?format=pdf")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("\"PDF content\"")
            .create_async()
            .await;

        let result = export_report(&client, 789, "pdf".to_string(), Some(file_path.clone())).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "PDF content");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_execute_metadata_action() {
        let (mut server, client) = setup_test_server().await;

        let mock = server
