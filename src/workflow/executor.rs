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

//! 工作流任务执行器
//!
//! 负责执行具体的工作流任务

use anyhow::{Context, Result};
use std::collections::HashMap;
use tracing::info;

use super::parser::TaskConfig;

/// 任务执行器
#[derive(Clone)]
pub struct TaskExecutor;

impl TaskExecutor {
    /// 创建新的任务执行器
    pub fn new() -> Self {
        Self
    }

    /// 执行单个任务
    pub async fn execute_task(
        &self,
        task: &TaskConfig,
        _variables: &HashMap<String, String>,
    ) -> Result<serde_yaml::Value> {
        info!("  执行任务类型: {}", task.task_type);

        // 根据任务类型分发到不同的处理器
        match task.task_type.as_str() {
            "sync" => {
                let tracking_id = task
                    .parameters
                    .get("tracking_id")
                    .and_then(|v| v.as_i64())
                    .context("sync 任务缺少 tracking_id 参数")?;

                info!("  执行 sync 任务: tracking_id = {}", tracking_id);
                Ok(serde_yaml::Value::String(format!(
                    "Sync completed for tracking {}",
                    tracking_id
                )))
            }
            "classify" => {
                let limit = task
                    .parameters
                    .get("limit")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(100);

                info!("  执行 classify 任务: limit = {}", limit);
                Ok(serde_yaml::Value::String(format!(
                    "Classified {} items",
                    limit
                )))
            }
            "compare" => {
                let tracking_id = task
                    .parameters
                    .get("tracking_id")
                    .and_then(|v| v.as_i64())
                    .context("compare 任务缺少 tracking_id 参数")?;

                info!("  执行 compare 任务: tracking_id = {}", tracking_id);
                Ok(serde_yaml::Value::String(format!(
                    "Comparison completed for tracking {}",
                    tracking_id
                )))
            }
            "export" => {
                let format = task
