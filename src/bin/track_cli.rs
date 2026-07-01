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

//! Track-System CLI 工具
//!
//! 提供生产级的命令行接口，用于管理和操作 Track-System
//!
//! 注意：这是纯客户端工具，通过 HTTP API 与 track-server 通信
//! 不再直接连接数据库

use anyhow::Result;
use clap::CommandFactory;
use track_system::cli::{Cli, CliExecutor};
use track_system::i18n::{apply_clap_i18n, apply_help_i18n, detect_lang_from_args, init_i18n};

#[tokio::main]
async fn main() -> Result<()> {
    let raw_args: Vec<String> = std::env::args().collect();
    let arg_lang = detect_lang_from_args(&raw_args);
    let locale = init_i18n(arg_lang.as_deref());

    let mut cmd = Cli::command();
    apply_clap_i18n(&mut cmd, "track_cli");
    apply_help_i18n(&mut cmd, "track_cli", &locale);
    let matches = cmd.get_matches_from(raw_args);
    let cli = <Cli as clap::FromArgMatches>::from_arg_matches(&matches).unwrap();
    init_i18n(cli.lang.as_deref());

    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(&cli.log_level)
        .without_time()
        .init();

    // 创建 CLI 执行器（纯客户端，不连接数据库）
    let executor = CliExecutor::new()?;

    // 执行命令
    executor.execute(cli).await?;

    Ok(())
}
