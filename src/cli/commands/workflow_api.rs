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
