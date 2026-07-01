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
        duration_ms: i32,
    ) -> Result<(), sea_orm::DbErr> {
        let action = audit_logs::AuditAction::from_method_and_path(method, path);
        let (resource_type, resource_id) = extract_resource_info(path);

        let audit_log = audit_logs::ActiveModel {
            user_id: Set(user_id),
            action: Set(action.to_string()),
            resource_type: Set(resource_type),
            resource_id: Set(resource_id),
            method: Set(method.to_string()),
            path: Set(path.to_string()),
            ip_address: Set(ip_address),
            user_agent: Set(user_agent),
            request_body: Set(None),
            response_status: Set(response_status),
            response_body: Set(None),
            duration: Set(Some(duration_ms)),
            error_message: Set(None),
            created_at: Set(Utc::now()),
            ..Default::default()
        };

        audit_log.insert(self.db).await?;

        Ok(())
    }

    /// 记录数据变更
    pub async fn log_data_change(
        &self,
        user_id: Option<String>,
        action: audit_logs::AuditAction,
        resource_type: &str,
        resource_id: &str,
        details: Option<serde_json::Value>,
    ) -> Result<(), sea_orm::DbErr> {
        let audit_log = audit_logs::ActiveModel {
            user_id: Set(user_id),
            action: Set(action.to_string()),
            resource_type: Set(resource_type.to_string()),
