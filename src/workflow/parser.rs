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

//! 工作流配置解析器
//!
//! 支持从 YAML 文件解析工作流定义

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// 工作流配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    /// 工作流名称
    pub name: String,

    /// 工作流描述
    #[serde(default)]
    pub description: String,

    /// 工作流版本
    #[serde(default = "default_version")]
    pub version: String,

    /// 工作流的任务列表
    pub tasks: Vec<TaskConfig>,

    /// 全局变量
    #[serde(default)]
    pub variables: HashMap<String, String>,

    /// 任务执行策略
    #[serde(default = "default_execution_policy")]
    pub execution_policy: ExecutionPolicy,
}

