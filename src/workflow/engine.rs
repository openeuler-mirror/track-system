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

//! 工作流引擎
//!
//! 负责协调任务执行和状态管理

use anyhow::Result;
use std::collections::HashMap;
use tracing::{error, info};

use super::executor::TaskExecutor;
use super::parser::{ExecutionPolicy, WorkflowConfig};

/// 任务执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

/// 工作流执行状态
pub struct WorkflowEngine {
    config: WorkflowConfig,
    task_status: HashMap<String, TaskStatus>,
    task_results: HashMap<String, serde_yaml::Value>,
}

impl WorkflowEngine {
    /// 创建新的工作流引擎
    pub fn new(config: WorkflowConfig) -> Result<Self> {
        config.validate()?;

