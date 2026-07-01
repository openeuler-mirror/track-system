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

//! Track-Server - 服务器
//!
//! 提供上游追踪能力和 RESTful API 接口
//!
//! 功能：
//! - 自动同步 L1/L2 仓库数据
//! - 定期执行同步、分类任务
//! - 提供 RESTful API 供客户端调用
//! - 数据分析和报告生成
//!
//! 运行模式：
//! 1. 服务器模式（默认）：Web API + 后台调度
//! 2. 调度器模式：仅后台调度

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use sea_orm::{ConnectOptions, Database};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::time::sleep;
use tracing::{error, info};

use track_system::collectors::{gitea::GiteaClient, gitee::GiteeClient};
use track_system::i18n::{apply_clap_i18n, apply_help_i18n, detect_lang_from_args, init_i18n};
use track_system::scheduler::{scheduler_manager::WakeSignal, SchedulerConfig, SchedulerManager};

#[derive(Parser)]
#[command(name = "track-server")]
#[command(about = "Track-System 服务器 - 提供追踪和API服务")]
#[command(about = "Track-System 调度器守护进程", long_about = None)]
#[command(version)]
struct Cli {
    /// 语言（zh-CN / en-US）
    #[arg(long, global = true)]
    lang: Option<String>,
    /// 数据库 URL
    #[arg(
        long,
        global = true,
        env = "DATABASE_URL",
        default_value = "sqlite:///var/lib/track-system/track-system.db?mode=rwc"
    )]
    database_url: String,

    /// 日志级别
    #[arg(long, global = true, default_value = "info")]
    log_level: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 仅运行调度器（不启动 Web 服务器）
    SchedulerOnly {
        /// 调度间隔（秒）
        #[arg(long, default_value = "3600")]
        interval: u64,

        /// 最大并发任务数
        #[arg(long, default_value = "10")]
        max_concurrent: usize,
    },

    /// 运行 Web 服务器 + 后台调度器
    Server {
        /// 服务器监听地址
        #[arg(long, default_value = "0.0.0.0:3000")]
        addr: String,

        /// 调度间隔（秒）
        #[arg(long, default_value = "3600")]
        interval: u64,

        /// 最大并发任务数
        #[arg(long, default_value = "10")]
        max_concurrent: usize,
    },

    /// 执行一次调度（不启动守护进程）
    RunOnce {
        /// 最大并发任务数
        #[arg(long, default_value = "10")]
        max_concurrent: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let raw_args: Vec<String> = std::env::args().collect();
    let arg_lang = detect_lang_from_args(&raw_args);
    let locale = init_i18n(arg_lang.as_deref());

    let mut cmd = Cli::command();
    apply_clap_i18n(&mut cmd, "track_server");
    apply_help_i18n(&mut cmd, "track_server", &locale);
    let matches = cmd.get_matches_from(raw_args);
    let cli = <Cli as clap::FromArgMatches>::from_arg_matches(&matches).unwrap();
    init_i18n(cli.lang.as_deref());

    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(&cli.log_level)
        .without_time()
        .with_ansi(false)
        .init();

    info!(" Track-System 调度器守护进程");
    info!("   版本: 0.1.0");
    info!("");

    // 连接数据库
    info!("连接数据库: {}", cli.database_url);
    let mut connect_opts = ConnectOptions::new(cli.database_url.clone());
    connect_opts
        .max_connections(20)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(10))
        .sqlx_logging(false);

    let db = Arc::new(Database::connect(connect_opts).await?);
    info!("数据库连接成功");

    // 根据命令执行不同模式
    match cli.command {
        Some(Commands::SchedulerOnly {
            interval,
            max_concurrent,
        }) => run_scheduler_only(db, interval, max_concurrent).await,
        Some(Commands::Server {
            addr,
            interval,
            max_concurrent,
        }) => run_server_with_scheduler(db, addr, interval, max_concurrent).await,
        Some(Commands::RunOnce { max_concurrent }) => run_once(db, max_concurrent).await,
        None => {
            // 默认：运行服务器 + 调度器
            run_server_with_scheduler(db, "0.0.0.0:3000".to_string(), 3600, 10).await
        }
    }
}

/// 仅运行调度器（不启动 Web 服务器）
async fn run_scheduler_only(
    db: Arc<sea_orm::DatabaseConnection>,
    interval_secs: u64,
    max_concurrent: usize,
) -> Result<()> {
    info!(" 启动调度器模式");
    info!("   调度间隔: {} 秒", interval_secs);
    info!("   最大并发: {} 任务", max_concurrent);
    info!("");

    let config = SchedulerConfig {
        max_concurrent_jobs: max_concurrent,
        job_timeout_secs: 1800,
        cleanup_interval_secs: interval_secs,
        health_check_interval_secs: 30,
    };

    let (mut scheduler, mut wake_rx) = SchedulerManager::new(db.clone(), None, config.clone());
    scheduler.start().await?;

    info!("调度器已启动");
    info!("");

    // 启动调度循环
    let scheduler_handle = tokio::spawn(async move {
        loop {
            info!("⚡ 执行调度轮次...");
            scheduler_only_execute_round_and_log(&scheduler).await;

            info!("");
            info!(
                "⏳ 等待 {} 秒后执行下一轮（可通过 API 手动唤醒）...",
                interval_secs
            );

            // 使用 select! 支持定时唤醒和手动唤醒
            tokio::select! {
                _ = sleep(Duration::from_secs(interval_secs)) => {
                    info!("⏰ 定时器触发，开始新一轮调度");
                }
                Some(signal) = wake_rx.recv() => {
                    if scheduler_only_handle_wake_signal(&scheduler, signal).await {
                        continue;
                    }
                }
            }
        }
    });

    // 等待信号
    info!(" 守护进程运行中，按 Ctrl+C 停止");
    info!("");

    match signal::ctrl_c().await {
        Ok(()) => {
            info!("");
            info!("🛑 收到停止信号，正在关闭...");
        }
        Err(err) => {
            error!(" 监听信号失败: {}", err);
        }
    }

    // 取消调度循环
    scheduler_handle.abort();

    info!("调度器守护进程已停止");

    Ok(())
}

/// 运行 Web 服务器 + 后台调度器
async fn run_server_with_scheduler(
    db: Arc<sea_orm::DatabaseConnection>,
    addr: String,
    interval_secs: u64,
    max_concurrent: usize,
) -> Result<()> {
    info!("🌐 启动服务器 + 调度器模式");
    info!("   服务器地址: {}", addr);
    info!("   调度间隔: {} 秒", interval_secs);
    info!("   最大并发: {} 任务", max_concurrent);
    info!("");

    let config = SchedulerConfig {
        max_concurrent_jobs: max_concurrent,
        job_timeout_secs: 1800,
        cleanup_interval_secs: interval_secs,
        health_check_interval_secs: 30,
    };

    // 创建调度器管理器并包装在 Arc<RwLock<>> 中
    let (mut scheduler, mut wake_rx) = SchedulerManager::new(db.clone(), None, config.clone());

    if let Err(err) = scheduler.start().await {
        error!(" 启动调度器失败: {}", err);
        return Err(err);
    }

    info!("后台调度器已启动");

    // 将调度器包装在 Arc<RwLock<>> 中，以便 Web 服务器和调度循环共享
    use tokio::sync::RwLock;
    let scheduler_manager = Arc::new(RwLock::new(scheduler));
    let scheduler_for_loop = scheduler_manager.clone();

    // 启动后台调度器
    let scheduler_config = config.clone();
    let scheduler_handle = tokio::spawn(async move {
        let mut wake_up = false;
        let mut tracking_id = None;
        loop {
            info!(" 执行调度轮次...");
            let scheduler = scheduler_for_loop.read().await;
            match scheduler.execute_round_wake_up(wake_up, tracking_id).await {
                Ok(results) => {
                    info!("调度轮次完成: 执行了 {} 个任务", results.len());
                }
                Err(err) => {
                    error!(" 调度轮次失败: {}", err);
                }
            }

            drop(scheduler); // 释放读锁

            wake_up = false;
            tracking_id = None;

            // 使用 select! 支持定时唤醒和手动唤醒
            tokio::select! {
                _ = sleep(Duration::from_secs(scheduler_config.cleanup_interval_secs)) => {}
                Some(signal) = wake_rx.recv() => {
                    server_loop_apply_wake_signal(signal, &mut wake_up, &mut tracking_id);
                }
            }
        }
    });

    // 构建 Web 应用，传递调度器管理器
    use track_system::server::{create_app_with_state, state::AppState};
    let (gitee, gitea) = load_external_clients_from_env();

    let state = AppState {
        db: db.clone(),
        gitee: gitee.map(Arc::new),
        gitea: gitea.map(Arc::new),
        scheduler_manager: Some(scheduler_manager),
    };

    let app = create_app_with_state(state);

    info!("Web 服务器已配置");
    info!("");
    info!(" 服务器运行中:");
    info!("   - Web API: http://{}/api", addr);
    info!("   - 健康检查: http://{}/api/health", addr);
    info!("   - 按 Ctrl+C 停止");
    info!("");

    // 启动服务器
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let server_handle = tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, app).await {
            error!(" 服务器错误: {}", err);
        }
    });

    // 等待信号
    match signal::ctrl_c().await {
        Ok(()) => {
            info!("");
            info!("🛑 收到停止信号，正在关闭...");
        }
        Err(err) => {
            error!(" 监听信号失败: {}", err);
        }
    }

    // 取消任务
    scheduler_handle.abort();
    server_handle.abort();

    info!("服务器和调度器已停止");

    Ok(())
}

/// 执行一次调度（不启动守护进程）
async fn run_once(db: Arc<sea_orm::DatabaseConnection>, max_concurrent: usize) -> Result<()> {
    info!(" 执行单次调度");
    info!("   最大并发: {} 任务", max_concurrent);
    info!("");

    let config = SchedulerConfig {
        max_concurrent_jobs: max_concurrent,
        job_timeout_secs: 1800,
        cleanup_interval_secs: 3600,
        health_check_interval_secs: 30,
    };

    let (mut scheduler, _wake_rx) = SchedulerManager::new(db, None, config);
    scheduler.start().await?;

    info!(" 开始执行...");

    match scheduler.execute_round().await {
        Ok(results) => {
            info!("");
            info!("调度完成: 执行了 {} 个任务", results.len());
            info!("");

            // 显示详细结果
            for (idx, result) in results.iter().enumerate() {
                if result.success {
                    info!("   {}. 任务 {} 成功", idx + 1, result.job_id);
                } else {
                    error!("   {}. 任务 {}  失败", idx + 1, result.job_id);
                }
            }

            info!("");
            info!("🎉 单次调度执行完成");
        }
        Err(err) => {
            error!(" 调度失败: {}", err);
            return Err(err);
        }
    }

    Ok(())
}

fn load_external_clients_from_env() -> (Option<GiteeClient>, Option<GiteaClient>) {
    use std::env;

    let gitee = env::var("GITEE_ACCESS_TOKEN")
        .ok()
        .and_then(|token| GiteeClient::new(token).ok());

    let gitea = match env::var("GITEA_ACCESS_TOKEN") {
        Ok(token) => {
            let base = env::var("GITEA_API_BASE")
                .unwrap_or_else(|_| "https://work.ctyun.cn/git/api/v1".to_string());
            GiteaClient::new(token, base).ok()
        }
        Err(_) => None,
    };

    (gitee, gitea)
}

async fn scheduler_only_execute_round_and_log(scheduler: &SchedulerManager) {
    match scheduler.execute_round().await {
        Ok(results) => {
            info!("调度轮次完成: 执行了 {} 个任务", results.len());

            for (idx, result) in results.iter().enumerate() {
                if result.success {
                    info!("   {}. 任务 {} 成功", idx + 1, result.job_id);
                } else {
                    error!("   {}. 任务 {} 失败", idx + 1, result.job_id);
                }
            }
        }
        Err(err) => {
            error!("⚠️ 调度轮次失败: {}", err);
        }
    }
}

async fn scheduler_only_handle_wake_signal(
    scheduler: &SchedulerManager,
    signal: WakeSignal,
) -> bool {
    match signal {
        WakeSignal::All => {
            info!("🔔 收到唤醒信号（所有任务），立即开始调度");
            false
        }
        WakeSignal::Specific(id) => {
            info!(
                tracking_id = id,
                "🔔 收到唤醒信号（指定任务），立即开始调度"
            );
            if let Err(e) = scheduler.execute_round_wake_up(true, Some(id)).await {
                error!(tracking_id = id, error = %e, "指定任务执行失败");
            }
            true
        }
    }
}

fn server_loop_apply_wake_signal(
    signal: WakeSignal,
    wake_up: &mut bool,
    tracking_id: &mut Option<i32>,
) {
    match signal {
        WakeSignal::All => {
            info!("🔔 收到唤醒信号（所有任务），立即开始调度");
            *wake_up = true;
        }
        WakeSignal::Specific(id) => {
            *tracking_id = Some(id);
            *wake_up = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use sea_orm::{DatabaseBackend, MockDatabase};
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn cli_default_command_is_none_when_no_subcommand() {
        let args = vec!["track-server"];
        let cli = Cli::parse_from(args);
        assert!(cli.command.is_none());
        assert_eq!(cli.log_level, "info");
        assert_eq!(
            cli.database_url,
            "sqlite:///var/lib/track-system/track-system.db?mode=rwc".to_string()
        );
    }

    #[test]
    fn cli_parse_scheduler_only_command() {
        let args = vec![
            "track-server",
            "--database-url",
            "sqlite://custom.db",
            "--log-level",
            "debug",
            "scheduler-only",
            "--interval",
            "120",
            "--max-concurrent",
            "5",
        ];
        let cli = Cli::parse_from(args);
        match cli.command {
            Some(Commands::SchedulerOnly {
                interval,
                max_concurrent,
            }) => {
                assert_eq!(interval, 120);
                assert_eq!(max_concurrent, 5);
                assert_eq!(cli.database_url, "sqlite://custom.db");
                assert_eq!(cli.log_level, "debug");
            }
            _ => panic!("expected SchedulerOnly"),
        }
    }

    #[test]
    fn cli_parse_server_command() {
        let args = vec![
            "track-server",
            "server",
            "--addr",
            "127.0.0.1:4000",
            "--interval",
            "600",
            "--max-concurrent",
            "20",
            "--database-url",
            "sqlite://custom.db",
            "--log-level",
            "debug",
        ];
        let cli = Cli::parse_from(args);
        match cli.command {
            Some(Commands::Server {
                addr,
                interval,
                max_concurrent,
            }) => {
                assert_eq!(addr, "127.0.0.1:4000");
                assert_eq!(interval, 600);
                assert_eq!(max_concurrent, 20);
                assert_eq!(cli.database_url, "sqlite://custom.db");
                assert_eq!(cli.log_level, "debug");
            }
            _ => panic!("expected Server"),
        }
    }

    #[test]
    fn cli_parse_run_once_command() {
        let args = vec!["track-server", "run-once", "--max-concurrent", "3"];
        let cli = Cli::parse_from(args);
        match cli.command {
            Some(Commands::RunOnce { max_concurrent }) => {
                assert_eq!(max_concurrent, 3);
            }
            _ => panic!("expected RunOnce"),
        }
    }

    #[tokio::test]
    async fn run_once_succeeds_when_no_pending_tasks() {
        use track_system::entities::{packages, tracking};

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<(tracking::Model, Option<packages::Model>), _, _>(vec![vec![]])
            .into_connection();
        let db = Arc::new(db);

        run_once(db, 3).await.unwrap();
    }

    #[tokio::test]
    async fn scheduler_only_execute_round_and_log_handles_ok_and_err() {
        use track_system::entities::{packages, tracking};

        let ok_db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<(tracking::Model, Option<packages::Model>), _, _>(vec![vec![]])
            .into_connection();
        let ok_db = Arc::new(ok_db);

        let (manager, _rx) = SchedulerManager::new(ok_db, None, SchedulerConfig::default());
        scheduler_only_execute_round_and_log(&manager).await;

        let err_db = Arc::new(MockDatabase::new(DatabaseBackend::Postgres).into_connection());
        let (manager, _rx) = SchedulerManager::new(err_db, None, SchedulerConfig::default());
        scheduler_only_execute_round_and_log(&manager).await;
    }

    #[tokio::test]
    async fn scheduler_only_handle_wake_signal_returns_skip_for_specific() {
        let db = Arc::new(MockDatabase::new(DatabaseBackend::Postgres).into_connection());
        let (manager, _rx) = SchedulerManager::new(db, None, SchedulerConfig::default());

        let skip = scheduler_only_handle_wake_signal(&manager, WakeSignal::All).await;
        assert!(!skip);

        let skip = scheduler_only_handle_wake_signal(&manager, WakeSignal::Specific(1)).await;
        assert!(skip);
    }

    #[test]
    fn server_loop_apply_wake_signal_sets_flags() {
        let mut wake_up = false;
        let mut tracking_id = None;

        server_loop_apply_wake_signal(WakeSignal::All, &mut wake_up, &mut tracking_id);
        assert!(wake_up);
        assert!(tracking_id.is_none());

        wake_up = false;
        server_loop_apply_wake_signal(WakeSignal::Specific(42), &mut wake_up, &mut tracking_id);
        assert!(wake_up);
        assert_eq!(tracking_id, Some(42));
    }

    #[test]
    fn load_external_clients_from_env_respects_env_vars() {
        let _guard = env_lock().lock().unwrap();

        std::env::remove_var("GITEE_ACCESS_TOKEN");
        std::env::remove_var("GITEA_ACCESS_TOKEN");
        std::env::remove_var("GITEA_API_BASE");
        let (gitee, gitea) = load_external_clients_from_env();
        assert!(gitee.is_none());
        assert!(gitea.is_none());

        std::env::set_var("GITEE_ACCESS_TOKEN", "t");
        std::env::set_var("GITEA_ACCESS_TOKEN", "t2");
        std::env::set_var("GITEA_API_BASE", "http://localhost/api/v1");
        let (gitee, gitea) = load_external_clients_from_env();
        assert!(gitee.is_some());
        assert!(gitea.is_some());
    }
}
