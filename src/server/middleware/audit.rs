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

//! 审计日志中间件
//!
//! 记录所有 API 调用的详细信息

use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tracing::error;

use crate::entities::audit_logs;

/// 审计日志中间件
pub struct AuditMiddleware;

impl AuditMiddleware {
    /// 创建审计日志中间件处理函数
    pub async fn log_request(
        State(db): State<Arc<DatabaseConnection>>,
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
        request: Request,
        next: Next,
    ) -> Response {
        let start = Instant::now();

        // 提取请求信息
        let method = request.method().to_string();
        let path = request.uri().path().to_string();
        let query = request.uri().query().map(|q| q.to_string());
        let user_agent = request
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let user_id: Option<String> = None;

        // 构建完整路径
        let full_path = if let Some(q) = query {
            format!("{}?{}", path, q)
        } else {
            path.clone()
        };

        // 确定操作类型和资源类型
        let action = audit_logs::AuditAction::from_method_and_path(&method, &path);
        let (resource_type, resource_id) = extract_resource_info(&path);

        // 执行请求
        let response = next.run(request).await;

        // 计算处理时长
        let duration = start.elapsed().as_millis() as i32;

        // 提取响应状态
        let status = response.status();
        let status_code = status.as_u16() as i32;

        // 记录审计日志
        if let Err(e) = log_audit(
            &db,
            user_id,
            action,
            resource_type,
            resource_id,
            method,
            full_path,
            addr.ip().to_string(),
            user_agent,
            status_code,
            duration,
        )
        .await
        {
            error!(error = %e, "记录审计日志失败");
        }

        response
    }
}

/// 从路径中提取资源类型和资源 ID
fn extract_resource_info(path: &str) -> (String, Option<String>) {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if parts.len() < 2 {
        return ("unknown".to_string(), None);
    }

    // 跳过 "api" 前缀
    let start_idx = if parts[0] == "api" { 1 } else { 0 };

    if start_idx >= parts.len() {
        return ("unknown".to_string(), None);
    }

    let resource_type = parts[start_idx].to_string();

    // 尝试提取资源 ID (通常是路径中的数字或最后一个部分)
    let resource_id = if parts.len() > start_idx + 1 {
        let potential_id = parts[start_idx + 1];
        // 检查是否是数字或看起来像 ID
        if potential_id.parse::<i64>().is_ok() || potential_id.len() < 50 {
            Some(potential_id.to_string())
        } else {
            None
        }
    } else {
        None
