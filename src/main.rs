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

use anyhow::Result;
use clap::Parser;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, Database};
use std::time::Duration;
use tracing::{error, info};
use track_system::cli::{parser::Cli, CliExecutor};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    init_tracing();

    // 解析命令行参数
    let cli = Cli::parse();

    info!(" Track System - Automated Repository Tracking");
    info!(" Version: 0.1.0");
    info!(" Developed by: Yong Qin <qiny15@chinatelecom.cn>");
    info!("");

    // 加载配置
    info!("  加载配置...");
    let config = load_config();
    info!(" 配置加载完成");

    // 连接数据库
    info!("  连接数据库: {}", config.database_url);
    let db = connect_database(&config.database_url).await?;
    info!(" 数据库连接成功");

    // 运行数据库迁移
    info!(" 运行数据库迁移...");
    if let Err(e) = run_migrations(&db).await {
        error!(" 数据库迁移失败: {}", e);
        return Err(e);
    }
    info!(" 数据库迁移完成");

    // 执行 CLI 命令
    info!("");
    let executor = CliExecutor::new()?;
    if let Err(e) = executor.execute(cli).await {
        error!(" 命令执行失败: {}", e);
        return Err(e);
    }

    info!(" 程序退出");

    Ok(())
}

/// 初始化日志系统
fn init_tracing() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let file_appender =
        tracing_appender::rolling::daily("/var/log/track-system", "track-server.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "track_system=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().without_time())
        .with(
            tracing_subscriber::fmt::layer()
                .without_time()
                .with_writer(non_blocking)
                .with_ansi(false),
        )
        .init();
}

/// 配置结构
#[derive(Clone)]
struct Config {
    database_url: String,
    #[allow(dead_code)]
    server_addr: String,
}

/// 加载配置
fn load_config() -> Config {
    // 优先从环境变量加载
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://data/track-system.db".to_string());

    let server_addr = std::env::var("SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());

    Config {
        database_url,
        server_addr,
    }
}

/// 连接数据库
async fn connect_database(database_url: &str) -> Result<sea_orm::DatabaseConnection> {
    let mut opt = ConnectOptions::new(database_url.to_owned());
    opt.max_connections(100)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(true);

    let db = Database::connect(opt).await?;
    Ok(db)
}

/// 运行数据库迁移
async fn run_migrations(db: &sea_orm::DatabaseConnection) -> Result<()> {
    Migrator::up(db, None).await?;
    Ok(())
}
