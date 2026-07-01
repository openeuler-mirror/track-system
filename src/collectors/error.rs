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

use thiserror::Error;

/// API 客户端错误类型
#[derive(Error, Debug)]
pub enum ApiError {
    /// HTTP 请求错误
    #[error("HTTP 请求失败: {0}")]
    HttpError(#[from] reqwest::Error),

    /// 认证错误（401/403）
    #[error("认证失败: {0}")]
    AuthenticationError(String),

    /// API 限流错误（429）
    #[error("API 限流: {0}")]
    RateLimitError(String),

    /// 资源不存在错误（404）
    #[error("资源不存在: {0}")]
    NotFoundError(String),

    /// 服务器错误（5xx）
    #[error("服务器错误: {0}")]
    ServerError(String),

    /// JSON 解析错误
    #[error("JSON 解析失败: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Base64 解码错误
    #[error("Base64 解码失败: {0}")]
    Base64Error(String),

    /// 配置错误
    #[error("配置错误: {0}")]
    InvalidConfig(String),

    /// 超时错误
    #[error("请求超时")]
    TimeoutError,

    /// 其他错误
    #[error("未知错误: {0}")]
    Unknown(String),
}

/// API 结果类型别名
pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    /// 从 HTTP 状态码创建错误
    pub fn from_status(status: u16, message: String) -> Self {
        match status {
            401 | 403 => Self::AuthenticationError(message),
            404 => Self::NotFoundError(message),
            429 => Self::RateLimitError(message),
            500..=599 => Self::ServerError(message),
            _ => Self::Unknown(message),
        }
    }

    /// 判断是否为可重试的错误
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::TimeoutError
                | Self::ServerError(_)
                | Self::RateLimitError(_)
                | Self::HttpError(_)
        )
    }
}

