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

//! Config 命令处理器（纯客户端配置管理）

use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::path::PathBuf;

use crate::cli::{client::ClientConfig, parser::ConfigAction};

/// 执行配置管理命令
pub async fn execute(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Init { path } => init_config(path).await,
        ConfigAction::Validate { path } => validate_config(path).await,
        ConfigAction::Show { section, format } => show_config(section, format).await,
    }
}

/// 初始化配置文件
async fn init_config(path: Option<String>) -> Result<()> {
    let config_path = get_config_path(path)?;

    if config_path.exists() {
        println!(
            "{}",
            format!("配置文件已存在: {}", config_path.display()).yellow()
        );
        print!("是否覆盖? (y/N): ");
        use std::io::{self, Write};
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("已取消");
            return Ok(());
        }
    }

    // 创建默认配置
    let default_config = ClientConfig {
        server_url: "http://localhost:3000".to_string(),
        auth_token: None,
        timeout: 30,
        verify_ssl: true,
    };

    // 确保目录存在
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // 写入配置文件
    let toml_content = toml::to_string_pretty(&default_config)?;
    fs::write(&config_path, toml_content)?;

    println!("{}", "✓ 配置文件已创建".green());
    println!("路径: {}", config_path.display());
    println!("\n默认配置:");
    println!("  服务器地址: {}", default_config.server_url);
    println!("  超时时间: {} 秒", default_config.timeout);
    println!("\n请使用 'track-cli server config' 命令配置服务器连接");

    Ok(())
}

/// 验证配置文件
async fn validate_config(path: Option<String>) -> Result<()> {
    let config_path = get_config_path(path)?;

    if !config_path.exists() {
