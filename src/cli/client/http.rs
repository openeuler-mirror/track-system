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
    }

    /// 从配置文件创建客户端
    pub fn from_config_file() -> ApiResult<Self> {
        let config = ClientConfig::from_env()?;
        Self::new(config)
    }

    /// 获取配置
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// 构建请求
    fn build_request(&self, method: Method, path: &str) -> RequestBuilder {
        let url = format!("{}{}", self.config.api_base_url(), path);
        let mut builder = self.client.request(method, &url);

        // 添加认证 token
        if let Some(token) = &self.config.auth_token {
            builder = builder.bearer_auth(token);
        }

        // 添加通用 headers
        builder = builder.header("Content-Type", "application/json");

        builder
    }

    /// 处理响应
    async fn handle_response<T: DeserializeOwned>(response: Response) -> ApiResult<T> {
        let status = response.status();

        if status.is_success() {
            // 成功响应
            let data = response
                .json::<T>()
                .await
                .map_err(|e| ApiError::JsonError(format!("解析响应失败: {}", e)))?;
            Ok(data)
        } else {
            // 错误响应
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "未知错误".to_string());

            // 尝试解析为标准错误格式
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&error_text) {
                Self::map_error_response(status, error_response)
            } else {
                Self::map_status_error(status, error_text)
            }
        }
    }

    /// 映射错误响应
    fn map_error_response<T>(status: StatusCode, error: ErrorResponse) -> ApiResult<T> {
        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                Err(ApiError::AuthenticationError(error.message))
            }
            StatusCode::NOT_FOUND => Err(ApiError::NotFound(error.message)),
            StatusCode::BAD_REQUEST => Err(ApiError::BadRequest(error.message)),
            _ => Err(ApiError::ServerError {
                status: status.as_u16(),
                message: error.message,
            }),
        }
    }

    /// 映射状态码错误
    fn map_status_error<T>(status: StatusCode, message: String) -> ApiResult<T> {
        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                Err(ApiError::AuthenticationError(message))
            }
            StatusCode::NOT_FOUND => Err(ApiError::NotFound(message)),
            StatusCode::BAD_REQUEST => Err(ApiError::BadRequest(message)),
            _ => Err(ApiError::ServerError {
                status: status.as_u16(),
                message,
            }),
        }
    }

    /// GET 请求
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> ApiResult<T> {
        let response = self
            .build_request(Method::GET, path)
            .send()
            .await
            .map_err(ApiError::from)?;

        Self::handle_response(response).await
    }

    /// POST 请求
    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> ApiResult<T> {
        let response = self
            .build_request(Method::POST, path)
            .json(body)
            .send()
            .await
            .map_err(ApiError::from)?;

        Self::handle_response(response).await
    }

    /// PUT 请求
    pub async fn put<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> ApiResult<T> {
        let response = self
            .build_request(Method::PUT, path)
            .json(body)
            .send()
            .await
            .map_err(ApiError::from)?;

        Self::handle_response(response).await
    }

    /// DELETE 请求
    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> ApiResult<T> {
        let response = self
            .build_request(Method::DELETE, path)
            .send()
            .await
            .map_err(ApiError::from)?;

        Self::handle_response(response).await
    }

    /// DELETE 请求（无响应体）
    pub async fn delete_no_content(&self, path: &str) -> ApiResult<()> {
        let response = self
            .build_request(Method::DELETE, path)
            .send()
            .await
            .map_err(ApiError::from)?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "未知错误".to_string());
            Self::map_status_error(status, error_text)
        }
    }

    /// 健康检查
    pub async fn health_check(&self) -> ApiResult<serde_json::Value> {
        self.get("/health").await
    }

    /// 测试连接
    pub async fn ping(&self) -> ApiResult<bool> {
        match self.health_check().await {
            Ok(_) => Ok(true),
            Err(ApiError::NetworkError(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

