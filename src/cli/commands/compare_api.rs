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

//! 对比分析命令实现（基于 API）
//!
//! 通过 HTTP API 执行对比分析

use anyhow::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::cli::client::ApiClient;
use crate::cli::formatter::format_datetime_local;
use crate::cli::parser::CompareAction;

/// 对比任务状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CompareStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// 对比任务响应
#[derive(Debug, Serialize, Deserialize)]
struct CompareTaskResponse {
    task_id: String,
    status: CompareStatus,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// 对比状态响应
