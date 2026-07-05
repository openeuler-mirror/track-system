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

//! Workflow 命令处理器（通过 API）

use anyhow::Result;
use colored::Colorize;
use std::fs;

use crate::cli::{client::ApiClient, parser::WorkflowAction};

/// 执行工作流命令
pub async fn execute(api_client: &ApiClient, action: WorkflowAction) -> Result<()> {
    match action {
        WorkflowAction::Execute { workflow_file, var } => {
            execute_workflow(api_client, workflow_file, var).await
        }
        WorkflowAction::List => list_workflows(api_client).await,
        WorkflowAction::Validate { workflow_file } => {
            validate_workflow(api_client, workflow_file).await
        }
        WorkflowAction::DryRun { workflow_file, var } => {
            dry_run_workflow(api_client, workflow_file, var).await
        }
    }
}

/// 执行工作流
async fn execute_workflow(
    api_client: &ApiClient,
    workflow_file: String,
    vars: Vec<String>,
) -> Result<()> {
    println!("{}", format!("正在执行工作流: {}...", workflow_file).cyan());

    // 读取工作流文件
    let workflow_content = fs::read_to_string(&workflow_file)?;

    // 解析变量
    let mut variables = std::collections::HashMap::new();
    for var in vars {
        let parts: Vec<&str> = var.splitn(2, '=').collect();
        if parts.len() == 2 {
            variables.insert(parts[0].to_string(), parts[1].to_string());
        }
    }

    let result: serde_json::Value = api_client
        .post(
            "/workflow/execute",
            &serde_json::json!({
                "workflow": workflow_content,
                "variables": variables
            }),
        )
        .await?;

    println!("{}", "✓ 工作流已提交".green());
    println!("执行 ID: {}", result["execution_id"]);
    println!("状态: {}", result["status"]);

    Ok(())
}

/// 列出所有可用的工作流
async fn list_workflows(api_client: &ApiClient) -> Result<()> {
    println!("{}", "正在获取工作流列表...".cyan());

    let result: serde_json::Value = api_client.get("/workflow/list").await?;
    let workflows = result["workflows"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("无效的响应格式"))?;

    if workflows.is_empty() {
        println!("{}", "没有可用的工作流".yellow());
        return Ok(());
    }

    println!("\n{}", "=== 可用工作流 ===".bold());
    for workflow in workflows {
        let name = workflow["name"].as_str().unwrap_or("unknown");
        let description = workflow["description"].as_str().unwrap_or("");
        println!("- {}: {}", name.green(), description);
    }

    Ok(())
}

/// 验证工作流定义
async fn validate_workflow(api_client: &ApiClient, workflow_file: String) -> Result<()> {
    println!("{}", format!("正在验证工作流: {}...", workflow_file).cyan());

    // 读取工作流文件
    let workflow_content = fs::read_to_string(&workflow_file)?;

    let result: serde_json::Value = api_client
        .post(
            "/workflow/validate",
            &serde_json::json!({
                "workflow": workflow_content
            }),
        )
        .await?;

    if result["valid"].as_bool().unwrap_or(false) {
        println!("{}", "✓ 工作流定义有效".green());
    } else {
        println!("{}", "✗ 工作流定义无效".red());
        if let Some(errors) = result["errors"].as_array() {
            println!("\n错误:");
            for error in errors {
                println!("  - {}", error.as_str().unwrap_or("unknown error"));
            }
        }
    }

    Ok(())
}

/// 模拟运行工作流
async fn dry_run_workflow(
    api_client: &ApiClient,
    workflow_file: String,
    vars: Vec<String>,
) -> Result<()> {
    println!(
        "{}",
        format!("正在模拟运行工作流: {}...", workflow_file).cyan()
    );

    // 读取工作流文件
    let workflow_content = fs::read_to_string(&workflow_file)?;

    // 解析变量
    let mut variables = std::collections::HashMap::new();
    for var in vars {
        let parts: Vec<&str> = var.splitn(2, '=').collect();
        if parts.len() == 2 {
            variables.insert(parts[0].to_string(), parts[1].to_string());
        }
    }

    let result: serde_json::Value = api_client
        .post(
            "/workflow/dry-run",
            &serde_json::json!({
                "workflow": workflow_content,
                "variables": variables
            }),
        )
        .await?;

    println!("{}", "✓ 模拟运行完成".green());
    println!("\n执行计划:");
    if let Some(steps) = result["steps"].as_array() {
        for (i, step) in steps.iter().enumerate() {
            println!(
                "  {}. {}",
                i + 1,
                step["name"].as_str().unwrap_or("unknown")
            );
            println!(
                "     操作: {}",
                step["action"].as_str().unwrap_or("unknown")
            );
        }
    }

    Ok(())
}

