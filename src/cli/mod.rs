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
pub struct CliExecutor {
    /// API 客户端
    api_client: ApiClient,
}

impl CliExecutor {
    /// 创建新的CLI执行器
    pub fn new() -> Result<Self> {
        let api_client = ApiClient::from_config_file()
            .map_err(|e| anyhow::anyhow!("初始化 API 客户端失败: {}", e))?;

        Ok(Self { api_client })
    }

    /// 使用指定配置创建 CLI 执行器
    pub fn with_config(config: ClientConfig) -> Result<Self> {
        let api_client =
            ApiClient::new(config).map_err(|e| anyhow::anyhow!("初始化 API 客户端失败: {}", e))?;

        Ok(Self { api_client })
    }

    /// 获取 API 客户端引用
    pub fn api_client(&self) -> &ApiClient {
        &self.api_client
    }

    /// 执行CLI命令
    pub async fn execute(&self, cli: Cli) -> Result<()> {
        match cli.command {
            parser::Commands::Sync { action } => {
                commands::sync_api::execute(&self.api_client, action).await
            }
            parser::Commands::Classify { action } => {
                commands::classify_api::execute(&self.api_client, action).await
            }
            parser::Commands::Workflow { action } => {
                commands::workflow_api::execute(&self.api_client, action).await
            }
            parser::Commands::L0 { action } => {
                commands::l0_api::execute(&self.api_client, action).await
            }
            parser::Commands::Compare { action } => {
                commands::compare_api::execute(&self.api_client, action).await
            }
            parser::Commands::Snapshot { action } => {
                commands::snapshot_api::execute(&self.api_client, action).await
            }
            parser::Commands::Export { action } => {
                commands::export_api::execute(&self.api_client, action).await
            }
            parser::Commands::Import { action } => {
                commands::import_api::execute(&self.api_client, action).await
            }
            parser::Commands::Config { action } => commands::config_api::execute(action).await,
            parser::Commands::Db { action: _ } => Ok(()),
            parser::Commands::Package { action } => {
                commands::package_api::execute(&self.api_client, action).await
            }
            parser::Commands::Distro { action } => {
                commands::distro_api::execute(&self.api_client, action).await
            }
            parser::Commands::Tracking { action } => {
                commands::tracking_api::execute(&self.api_client, action).await
            }
            parser::Commands::Status { action } => {
                commands::status_api::execute(&self.api_client, action).await
            }
            parser::Commands::Health { action } => {
                commands::health_api::execute(&self.api_client, action).await
