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
        anyhow::bail!("配置文件不存在: {}", config_path.display());
    }

    println!("{}", "正在验证配置文件...".cyan());

    // 读取并解析配置
    let content = fs::read_to_string(&config_path)?;
    let config: ClientConfig = toml::from_str(&content)?;

    println!("{}", "✓ 配置文件格式正确".green());
    println!("\n配置内容:");
    println!("  服务器地址: {}", config.server_url);
    println!(
        "  认证 Token: {}",
        if config.auth_token.is_some() {
            "已配置".green()
        } else {
            "未配置".yellow()
        }
    );
    println!("  超时时间: {} 秒", config.timeout);

    // 验证服务器地址格式
    if !config.server_url.starts_with("http://") && !config.server_url.starts_with("https://") {
        println!(
            "\n{}: 服务器地址应以 http:// 或 https:// 开头",
            "警告".yellow()
        );
    }

    Ok(())
}

/// 显示配置
async fn show_config(section: Option<String>, format: String) -> Result<()> {
    let config_path = get_default_config_path()?;

    if !config_path.exists() {
        anyhow::bail!(
            "配置文件不存在: {}\n请先运行 'track-cli config init' 初始化配置",
            config_path.display()
        );
    }

    // 读取配置
    let content = fs::read_to_string(&config_path)?;
    let config: ClientConfig = toml::from_str(&content)?;

    match format.as_str() {
        "json" => show_config_json(&config, section)?,
        "yaml" => show_config_yaml(&config, section)?,
        "toml" => show_config_toml(&config, section)?,
        _ => anyhow::bail!("不支持的格式: {}", format),
    }

    Ok(())
}

/// 以 JSON 格式显示配置
fn show_config_json(config: &ClientConfig, section: Option<String>) -> Result<()> {
    if let Some(sec) = section {
        let value = match sec.as_str() {
            "server" | "server_url" => serde_json::json!({ "server_url": config.server_url }),
            "token" | "auth_token" => serde_json::json!({ "auth_token": config.auth_token }),
            "timeout" => serde_json::json!({ "timeout": config.timeout }),
            _ => anyhow::bail!("未知的配置部分: {}", sec),
        };
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&config)?);
    }
    Ok(())
}

/// 以 YAML 格式显示配置
fn show_config_yaml(config: &ClientConfig, section: Option<String>) -> Result<()> {
    if let Some(sec) = section {
        let value = match sec.as_str() {
            "server" | "server_url" => serde_yaml::to_string(&config.server_url)?,
            "token" | "auth_token" => serde_yaml::to_string(&config.auth_token)?,
            "timeout" => serde_yaml::to_string(&config.timeout)?,
            _ => anyhow::bail!("未知的配置部分: {}", sec),
        };
        print!("{}", value);
    } else {
        print!("{}", serde_yaml::to_string(&config)?);
    }
    Ok(())
}

/// 以 TOML 格式显示配置
fn show_config_toml(config: &ClientConfig, section: Option<String>) -> Result<()> {
    if let Some(sec) = section {
        match sec.as_str() {
            "server" | "server_url" => println!("server_url = \"{}\"", config.server_url),
            "token" | "auth_token" => {
                if let Some(token) = &config.auth_token {
                    println!("auth_token = \"{}\"", token);
                } else {
                    println!("# auth_token = \"\"");
                }
            }
            "timeout" => println!("timeout = {}", config.timeout),
            _ => anyhow::bail!("未知的配置部分: {}", sec),
        }
    } else {
        print!("{}", toml::to_string_pretty(&config)?);
    }
    Ok(())
}

/// 获取配置文件路径
fn get_config_path(path: Option<String>) -> Result<PathBuf> {
    if let Some(p) = path {
        Ok(PathBuf::from(p))
    } else {
        get_default_config_path()
    }
}

/// 获取默认配置文件路径
fn get_default_config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("无法获取用户主目录"))?;
    Ok(home.join(".track-cli").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_init_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let result = init_config(Some(config_path.to_str().unwrap().to_string())).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
        assert!(config_path.exists());

        // 验证配置内容
        let content = fs::read_to_string(&config_path).unwrap();
        let config: ClientConfig = toml::from_str(&content).unwrap();
        assert_eq!(config.server_url, "http://localhost:3000");
        assert_eq!(config.timeout, 30);
        assert!(config.verify_ssl);
    }

    #[tokio::test]
    async fn test_validate_config_valid() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        // 创建有效配置文件
        let config = ClientConfig {
            server_url: "http://localhost:8080".to_string(),
            auth_token: Some("test_token".to_string()),
            timeout: 60,
            verify_ssl: true,
        };
        let content = toml::to_string_pretty(&config).unwrap();
        fs::write(&config_path, content).unwrap();

        let result = validate_config(Some(config_path.to_str().unwrap().to_string())).await;
        assert!(result.is_ok(), "Result failed: {:?}", result.err());
    }
