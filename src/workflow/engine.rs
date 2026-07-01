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

        let mut task_status = HashMap::new();
        for task in &config.tasks {
            task_status.insert(task.name.clone(), TaskStatus::Pending);
        }

        Ok(Self {
            config,
            task_status,
            task_results: HashMap::new(),
        })
    }

    /// 从 YAML 文件创建工作流引擎
    pub fn from_file(path: &str) -> Result<Self> {
        let config = WorkflowConfig::from_file(path)?;
        Self::new(config)
    }

    /// 执行工作流
    pub async fn execute(&mut self, executor: &TaskExecutor) -> Result<()> {
        info!("开始执行工作流: {}", self.config.name);
        info!("工作流版本: {}", self.config.version);
        info!("任务总数: {}", self.config.tasks.len());
        info!("执行策略: {:?}", self.config.execution_policy);
        info!("");

        match self.config.execution_policy {
            ExecutionPolicy::Sequential => self.execute_sequential(executor).await,
            ExecutionPolicy::Parallel => self.execute_parallel(executor).await,
            ExecutionPolicy::DAG => self.execute_dag(executor).await,
        }
    }

    /// 顺序执行工作流任务
    async fn execute_sequential(&mut self, executor: &TaskExecutor) -> Result<()> {
        for task in &self.config.tasks.clone() {
            info!("执行任务: {}", task.name);

            // 检查是否所有依赖都已完成
            for dep in &task.depends_on {
                if let Some(TaskStatus::Success) = self.task_status.get(dep) {
                    continue;
                } else {
                    info!("    跳过任务，依赖 {} 未完成", dep);
