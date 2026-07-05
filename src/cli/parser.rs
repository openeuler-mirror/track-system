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
        #[command(subcommand)]
        action: WorkflowAction,
    },

    /// L0轮询相关命令
    #[command(hide = true)]
    L0 {
        #[command(subcommand)]
        action: L0Action,
    },

    /// 对比分析相关命令
    #[command(hide = true)]
    Compare {
        #[command(subcommand)]
        action: CompareAction,
    },

    /// 快照管理相关命令
    Snapshot {
        #[command(subcommand)]
        action: SnapshotAction,
    },

    /// 数据导出相关命令
    #[command(hide = true)]
    Export {
        #[command(subcommand)]
        action: ExportAction,
    },

    /// 数据导入相关命令
    Import {
        #[command(subcommand)]
        action: ImportAction,
    },

    /// 配置管理相关命令
    #[command(hide = true)]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// 数据库管理命令
    #[command(hide = true)]
    Db {
        #[command(subcommand)]
        action: DbAction,
    },

    /// 软件包管理命令
    Package {
        #[command(subcommand)]
        action: PackageAction,
    },

    /// 发行版管理命令
    #[command(hide = true)]
    Distro {
        #[command(subcommand)]
        action: DistroAction,
    },

    /// 跟踪配置管理命令
    Tracking {
        #[command(subcommand)]
        action: TrackingAction,
    },

    /// 系统状态查询命令
    #[command(hide = true)]
    Status {
        #[command(subcommand)]
        action: StatusAction,
    },

    /// 健康检查命令
    #[command(hide = true)]
    Health {
        #[command(subcommand)]
        action: HealthAction,
    },

    /// 服务器管理命令
    #[command(hide = true)]
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },

    /// 报告管理命令
    Report {
        #[command(subcommand)]
        action: ReportAction,
    },
}

// ============== Sync Commands ==============

#[derive(Subcommand, Debug)]
pub enum SyncAction {
    /// 执行单个tracking的数据同步
    #[command(about = "Run sync for a specific tracking")]
    Run {
        /// Tracking ID
        tracking_id: i32,

        /// 超时时间（秒）
        #[arg(long, default_value = "3600")]
        timeout: u64,

        /// 失败是否继续
        #[arg(long)]
        continue_on_error: bool,
    },

    /// 执行所有待处理的同步任务
    #[command(about = "Run sync for all pending trackings")]
    RunAll {
        /// 最大并发数
        #[arg(long, default_value = "4")]
        concurrency: usize,
    },

    /// 批量执行指定的tracking
    #[command(about = "Run sync for multiple trackings")]
    Batch {
        /// Tracking IDs
        ids: Vec<i32>,

        /// 最大并发数
        #[arg(long, default_value = "4")]
        concurrency: usize,
    },

    /// 唤醒调度器，立即触发调度
    #[command(about = "Wake up scheduler to trigger immediate scheduling")]
    Wake {
        /// 指定 tracking ID（可选，不指定则唤醒整个调度器）
        #[arg(long)]
        tracking_id: Option<i32>,
    },

    /// 显示同步状态
    #[command(about = "Show sync status")]
    Status,
}

// ============== Classify Commands ==============

#[derive(Subcommand, Debug)]
pub enum ClassifyAction {
    /// 处理待分类的commits
    #[command(about = "Process pending classification jobs")]
    Process {
        /// 处理数量限制
