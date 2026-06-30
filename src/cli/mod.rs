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

//! Track-System 统一CLI模块
//!
//! 提供统一的命令行接口，整合所有子命令功能
//!
//! 注意：CLI 现在是纯客户端，不再直接连接数据库
//! 所有操作通过 HTTP API 与 track-server 通信

pub mod client;
pub mod commands;
pub mod dto;
pub mod formatter;
pub mod parser;
pub mod services;

pub use client::{ApiClient, ClientConfig};
pub use commands::*;
pub use dto::*;
pub use parser::Cli;

use anyhow::Result;

/// CLI 执行器（纯客户端版本）
///
/// 不再持有数据库连接，所有操作通过 HTTP API 完成
