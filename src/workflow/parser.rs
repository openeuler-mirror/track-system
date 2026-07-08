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

/// 任务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    /// 任务名称
    pub name: String,

    /// 任务类型 (sync, classify, compare, export, etc.)
    pub task_type: String,

    /// 任务参数
    #[serde(default)]
    pub parameters: HashMap<String, serde_yaml::Value>,

    /// 依赖的任务（前置任务）
    #[serde(default)]
    pub depends_on: Vec<String>,

    /// 重试策略
    #[serde(default)]
    pub retry: RetryConfig,

    /// 超时时间（秒）
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// 是否并行执行
    #[serde(default)]
    pub parallel: bool,

    /// 条件判断 (简单的表达式)
    #[serde(default)]
    pub condition: Option<String>,
}

/// 重试配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetryConfig {
    /// 最大重试次数
    #[serde(default = "default_max_retries")]
    pub max_attempts: u32,

    /// 重试间隔（秒）
    #[serde(default = "default_retry_interval")]
    pub interval: u64,

    /// 重试退避倍数
