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
