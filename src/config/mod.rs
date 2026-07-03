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

//! 配置管理模块

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// 主配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub api: ApiConfig,
    pub scheduler: SchedulerConfig,
    pub rate_limit: RateLimitConfig,
    pub packages: Vec<PackageConfig>,
    pub distros: Vec<DistroConfig>,
    pub trackings: Vec<TrackingConfig>,
    pub server: ServerConfig,
    pub logging: LoggingConfig,
}

/// 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(rename = "type")]
    pub db_type: String,
    pub sqlite: Option<SqliteConfig>,
    pub postgresql: Option<PostgresqlConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteConfig {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresqlConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
}

/// API 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub gitee: GiteeConfig,
    pub github: GithubConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeConfig {
    pub token: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubConfig {
    pub token: String,
    pub base_url: String,
}

/// 调度器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub max_concurrent_jobs: usize,
    pub job_timeout_secs: u64,
    pub cleanup_interval_secs: u64,
    pub health_check_interval_secs: u64,
}

/// 速率限制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub gitee_per_minute: u32,
    pub github_per_minute: u32,
    pub burst_size: u32,
}

/// 软件包配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageConfig {
    pub name: String,
    pub level: i32,
    pub sync_interval_hours: i32,
    pub l0_repo_url: String,
    pub description: Option<String>,
}

/// 发行版配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistroConfig {
    pub name: String,
    pub version: String,
}

/// 跟踪配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingConfig {
    pub package: String,
    pub distro: String,
    pub l1: L1Config,
    pub l2: L2Config,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1Config {
    pub branch: String,
    pub repo_owner: String,
    pub repo_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Config {
    pub branch: String,
    pub repo_path: String,
}

/// Web 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub log_level: String,
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub output: String,
    pub file_path: Option<String>,
}

impl Config {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("无法读取配置文件: {:?}", path.as_ref()))?;
        
        let config: Config = serde_yaml::from_str(&content)
            .context("无法解析配置文件")?;
        
        config.validate()?;
        
        Ok(config)
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        // 验证数据库配置
        match self.database.db_type.as_str() {
            "sqlite" => {
                if self.database.sqlite.is_none() {
                    anyhow::bail!("SQLite 配置缺失");
                }
            }
            "postgresql" => {
                if self.database.postgresql.is_none() {
                    anyhow::bail!("PostgreSQL 配置缺失");
                }
            }
            _ => anyhow::bail!("不支持的数据库类型: {}", self.database.db_type),
        }

        // 验证软件包配置
        if self.packages.is_empty() {
            anyhow::bail!("至少需要配置一个软件包");
        }

        // 验证发行版配置
        if self.distros.is_empty() {
            anyhow::bail!("至少需要配置一个发行版");
        }

        // 验证跟踪配置
        for tracking in &self.trackings {
            // 检查软件包是否存在
