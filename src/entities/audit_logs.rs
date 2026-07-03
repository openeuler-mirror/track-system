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
