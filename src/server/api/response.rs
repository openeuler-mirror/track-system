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

//! 统一的 API 响应格式

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

/// 统一的 API 响应结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// 响应状态码
    pub code: u16,
    /// 响应消息
    pub message: String,
    /// 响应数据（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    /// 创建成功响应
    pub fn success(data: T) -> Self {
        Self {
            code: 200,
            message: "Success".to_string(),
            data: Some(data),
        }
    }

    /// 创建成功响应（带自定义消息）
    pub fn success_with_message(message: impl Into<String>, data: T) -> Self {
        Self {
            code: 200,
            message: message.into(),
            data: Some(data),
        }
    }

    /// 创建创建成功响应（201）
    pub fn created(data: T) -> Self {
        Self {
            code: 201,
            message: "Created".to_string(),
            data: Some(data),
        }
    }

    /// 创建无内容响应（204）
    pub fn no_content() -> ApiResponse<()> {
        ApiResponse {
            code: 204,
            message: "No Content".to_string(),
            data: None,
        }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.code).unwrap_or(StatusCode::OK);
        (status, Json(self)).into_response()
    }
}

/// 分页响应结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    /// 数据列表
    pub items: Vec<T>,
    /// 总数
    pub total: u64,
    /// 当前页码（从 1 开始）
    pub page: u64,
    /// 每页大小
    pub page_size: u64,
    /// 总页数
    pub total_pages: u64,
}

impl<T> PaginatedResponse<T> {
    /// 创建分页响应
    pub fn new(items: Vec<T>, total: u64, page: u64, page_size: u64) -> Self {
        let total_pages = if page_size > 0 {
            total.div_ceil(page_size)
