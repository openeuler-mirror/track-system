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

//! 客户端配置管理
//!
//! 管理 track-cli 的配置文件，包括服务器地址、认证 token 等

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use super::error::{ApiError, ApiResult};

/// 客户端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// 服务器地址
    pub server_url: String,

    /// 认证 token（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,

    /// 请求超时时间（秒）
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// 是否验证 SSL 证书
    #[serde(default = "default_verify_ssl")]
    pub verify_ssl: bool,
}

fn default_timeout() -> u64 {
    30
}

fn default_verify_ssl() -> bool {
    true
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:3000".to_string(),
            auth_token: None,
            timeout: default_timeout(),
            verify_ssl: default_verify_ssl(),
        }
    }
}

impl ClientConfig {
    /// 获取配置文件路径
    pub fn config_path() -> ApiResult<PathBuf> {
        // 优先查找用户主目录下的配置
        let home = dirs::home_dir();
        if let Some(home_dir) = home {
            let user_config = home_dir.join(".track-cli").join("config.toml");
            if user_config.exists() {
                return Ok(user_config);
            }
        }

        // 其次查找系统级配置
        let system_config = PathBuf::from("/etc/track-system/track-cli.toml");
        if system_config.exists() {
            return Ok(system_config);
        }

        // 如果都不存在，返回用户主目录下的路径（用于新建）
        dirs::home_dir()
            .map(|h| h.join(".track-cli").join("config.toml"))
            .ok_or_else(|| ApiError::ConfigError("无法获取用户主目录".to_string()))
    }

    /// 从配置文件加载
    pub fn load() -> ApiResult<Self> {
        let config_path = Self::config_path()?;
