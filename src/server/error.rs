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

use axum::response::{IntoResponse, Response};
use std::fmt;

/// API 错误类型
#[derive(Debug)]
pub enum ApiError {
    /// 数据库错误
    DatabaseError(sea_orm::DbErr),
    /// 资源未找到
    NotFound(String),
    /// 请求参数无效
    BadRequest(String),
    /// 未授权
    Unauthorized(String),
    /// 禁止访问
    Forbidden(String),
    /// 内部服务器错误
    InternalError(String),
    /// 冲突（如唯一性约束违反）
    Conflict(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DatabaseError(e) => write!(f, "Database error: {}", e),
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
            Self::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            Self::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            Self::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            Self::InternalError(msg) => write!(f, "Internal error: {}", msg),
            Self::Conflict(msg) => write!(f, "Conflict: {}", msg),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<sea_orm::DbErr> for ApiError {
    fn from(err: sea_orm::DbErr) -> Self {
        // 检查是否是唯一性约束冲突
        if let sea_orm::DbErr::Exec(sea_orm::RuntimeErr::SqlxError(sqlx_err)) = &err {
            if let Some(db_err) = sqlx_err.as_database_error() {
                // SQLite 唯一性约束错误码
                if db_err.message().contains("UNIQUE constraint failed") {
                    return Self::Conflict("Resource already exists".to_string());
                }
            }
        }
