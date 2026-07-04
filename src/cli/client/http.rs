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

//! HTTP API 客户端实现
//!
//! 提供与 track-server RESTful API 通信的客户端

use reqwest::{Client, Method, RequestBuilder, Response, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;

use super::config::ClientConfig;
use super::error::{ApiError, ApiResult, ErrorResponse};

/// API 客户端
#[derive(Debug, Clone)]
pub struct ApiClient {
    /// HTTP 客户端
    client: Client,
    /// 客户端配置
    config: ClientConfig,
}

impl ApiClient {
    /// 创建新的 API 客户端
    pub fn new(config: ClientConfig) -> ApiResult<Self> {
        config.validate()?;

        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout))
            .danger_accept_invalid_certs(!config.verify_ssl)
            .build()
            .map_err(|e| ApiError::ConfigError(format!("创建 HTTP 客户端失败: {}", e)))?;

        Ok(Self { client, config })
