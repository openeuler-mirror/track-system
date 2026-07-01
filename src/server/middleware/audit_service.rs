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

//! 审计日志服务
//!
//! 提供审计日志记录功能,可在 handler 中手动调用

use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};

use crate::entities::audit_logs;

/// 审计日志服务
pub struct AuditService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> AuditService<'a> {
    /// 创建新的审计日志服务
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// 记录 API 调用
    #[allow(clippy::too_many_arguments)]
    pub async fn log_api_call(
        &self,
        user_id: Option<String>,
        method: &str,
        path: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
        response_status: i32,
