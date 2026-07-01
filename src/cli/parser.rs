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

//! 命令行参数解析

use clap::{Parser, Subcommand};

/// Track-System: 统一的仓库追踪自动化平台
#[derive(Parser, Debug)]
#[command(name = "track-system")]
#[command(about = "Unified repository tracking and automation platform")]
#[command(version)]
#[command(author)]
pub struct Cli {
    /// 语言（zh-CN / en-US）
    #[arg(long, global = true)]
    pub lang: Option<String>,

    /// 日志级别 (debug, info, warn, error)
    #[arg(long, global = true, default_value = "info")]
    pub log_level: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 数据同步相关命令
    Sync {
        #[command(subcommand)]
        action: SyncAction,
    },

    /// 变更分类相关命令
    #[command(hide = true)]
    Classify {
        #[command(subcommand)]
        action: ClassifyAction,
    },

    /// 工作流相关命令
    #[command(hide = true)]
    Workflow {
