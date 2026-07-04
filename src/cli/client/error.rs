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

//! API 客户端错误处理
//!
//! 定义 API 调用过程中可能出现的错误类型

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// API 错误类型
#[derive(Error, Debug)]
pub enum ApiError {
    /// 网络连接错误
    #[error("网络连接失败: {0}")]
    NetworkError(String),

    /// HTTP 请求错误
    #[error("HTTP 请求失败: {0}")]
    RequestError(String),

    /// 服务器返回错误
    #[error("服务器错误 ({status}): {message}")]
    ServerError { status: u16, message: String },

    /// 认证失败
    #[error("认证失败: {0}")]
    AuthenticationError(String),

    /// 资源未找到
    #[error("资源未找到: {0}")]
    NotFound(String),

    /// 请求参数错误
    #[error("请求参数错误: {0}")]
    BadRequest(String),

    /// JSON 序列化/反序列化错误
    #[error("JSON 处理错误: {0}")]
    JsonError(String),

    /// 配置错误
    #[error("配置错误: {0}")]
    ConfigError(String),

    /// 超时错误
    #[error("请求超时")]
    Timeout,

    /// 其他错误
    #[error("未知错误: {0}")]
    Other(String),
}

/// API 响应结果类型
pub type ApiResult<T> = Result<T, ApiError>;

/// 标准 API 错误响应格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl From<reqwest::Error> for ApiError {
    fn from(err: reqwest::Error) -> Self {
