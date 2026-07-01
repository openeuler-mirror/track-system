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

//! 审计日志实体

use sea_orm::{entity::prelude::*, JsonValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "audit_logs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 用户 ID (可选,未认证的请求为 None)
    pub user_id: Option<String>,
    /// 操作类型 (create, read, update, delete, execute)
    pub action: String,
    /// 资源类型 (package, tracking, report, etc.)
    pub resource_type: String,
    /// 资源 ID (可选)
    pub resource_id: Option<String>,
    /// HTTP 方法 (GET, POST, PUT, DELETE, etc.)
    pub method: String,
    /// 请求路径
    pub path: String,
    /// 客户端 IP 地址
    pub ip_address: Option<String>,
    /// User-Agent
    #[sea_orm(column_type = "Text", nullable)]
    pub user_agent: Option<String>,
    /// 请求体 (JSON)
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub request_body: Option<JsonValue>,
    /// 响应状态码
    pub response_status: i32,
    /// 响应体 (JSON, 可选)
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub response_body: Option<JsonValue>,
    /// 请求处理时长 (毫秒)
    pub duration: Option<i32>,
    /// 错误信息 (如果有)
    #[sea_orm(column_type = "Text", nullable)]
    pub error_message: Option<String>,
    /// 创建时间
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// 审计日志操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditAction {
    Create,
    Read,
    Update,
    Delete,
    Execute,
    Login,
    Logout,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Read => "read",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Execute => "execute",
            Self::Login => "login",
            Self::Logout => "logout",
        }
    }

    pub fn from_method_and_path(method: &str, path: &str) -> Self {
        match method {
            "GET" => Self::Read,
            "POST" => {
                if path.contains("/login") {
                    Self::Login
                } else if path.contains("/execute") || path.contains("/compare") {
                    Self::Execute
                } else {
                    Self::Create
                }
            }
            "PUT" | "PATCH" => Self::Update,
            "DELETE" => Self::Delete,
            _ => Self::Read,
        }
    }
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
