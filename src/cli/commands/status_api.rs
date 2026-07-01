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

//! 系统状态查询命令实现（基于 API）
//!
//! 通过 HTTP API 查询系统状态

use anyhow::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::cli::client::ApiClient;
use crate::cli::parser::StatusAction;

/// 系统状态响应
#[derive(Debug, Serialize, Deserialize)]
struct SystemStatus {
    status: String,
    version: String,
    uptime: u64,
    database: DatabaseStatus,
    scheduler: SchedulerStatus,
}

/// 数据库状态
#[derive(Debug, Serialize, Deserialize)]
struct DatabaseStatus {
    connected: bool,
    pool_size: usize,
}

/// 调度器状态
#[derive(Debug, Serialize, Deserialize)]
struct SchedulerStatus {
    running: bool,
