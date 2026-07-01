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

//! 工作流调度器
//!
//! 支持定时执行工作流

use anyhow::Result;
use std::collections::HashMap;
use tracing::info;

use super::engine::WorkflowEngine;
use super::executor::TaskExecutor;

/// 工作流调度项
#[derive(Debug, Clone)]
pub struct WorkflowScheduleItem {
    pub name: String,
    pub workflow_path: String,
    pub cron_expression: String,
    pub enabled: bool,
}

/// 工作流调度器
pub struct WorkflowScheduler {
    items: HashMap<String, WorkflowScheduleItem>,
}

impl WorkflowScheduler {
    /// 创建新的工作流调度器
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    /// 添加工作流到调度
    pub fn add_workflow(&mut self, item: WorkflowScheduleItem) {
        self.items.insert(item.name.clone(), item);
    }

    /// 获取所有调度的工作流
    pub fn list_workflows(&self) -> Vec<&WorkflowScheduleItem> {
        self.items.values().collect()
    }

    /// 启动调度器
    pub async fn start(&self) -> Result<()> {
        info!("启动工作流调度器");
        info!("已注册的工作流: {}", self.items.len());

        for (name, item) in &self.items {
            if item.enabled {
                info!("  ✓ {}: {}", name, item.cron_expression);
            } else {
                info!("  ✗ {} (已禁用)", name);
            }
        }

        // 这里可以使用 tokio-cron-scheduler 或其他调度库

        Ok(())
    }

    /// 手动执行一个工作流
    pub async fn execute_workflow(&self, workflow_name: &str) -> Result<()> {
        let item = self
            .items
            .get(workflow_name)
            .ok_or_else(|| anyhow::anyhow!("工作流 {} 不存在", workflow_name))?;

        info!("手动执行工作流: {}", workflow_name);

        // 加载工作流
        let mut engine = WorkflowEngine::from_file(&item.workflow_path)?;

        // 创建执行器并执行
        let executor = TaskExecutor::new();
        engine.execute(&executor).await?;

        // 输出摘要
        let summary = engine.summary();
        info!("{}", summary);

        Ok(())
    }
}

impl Default for WorkflowScheduler {
    fn default() -> Self {
        Self::new()
    }
}

