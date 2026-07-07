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

//! 报告查询 API handlers

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::server::{
    api::{ApiResponse, PaginatedResponse},
    error::{ApiError, ApiResult},
    state::AppState,
};

/// 报告列表查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportListQuery {
    /// 页码（从 1 开始）
    pub page: Option<u64>,
    /// 每页大小
    pub page_size: Option<u64>,
    /// 按跟踪配置 ID 过滤
    pub tracking_id: Option<i32>,
    /// 按报告类型过滤
    pub report_type: Option<String>,
    /// 按状态过滤
    pub status: Option<String>,
}

/// 报告摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    /// 报告 ID
    pub id: i64,
    /// 跟踪配置 ID
    pub tracking_id: i32,
    /// 报告类型（l1_vs_l0, l2_vs_l1）
    pub report_type: String,
    /// 软件包名称
    pub package_name: String,
    /// 报告状态
    pub status: String,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// 报告详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportDetail {
    /// 报告 ID
    pub id: i64,
    /// 跟踪配置 ID
    pub tracking_id: i32,
    /// 报告类型
    pub report_type: String,
    /// 软件包名称
    pub package_name: String,
    /// 报告状态
    pub status: String,
    /// 报告内容（JSON）
    pub content: serde_json::Value,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// 报告导出格式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Json,
    Yaml,
    Markdown,
    Html,
}

/// GET /api/reports
///
/// 查询报告列表（支持分页和过滤）
pub async fn list_reports(
    State(state): State<AppState>,
    Query(query): Query<ReportListQuery>,
) -> ApiResult<Json<ApiResponse<PaginatedResponse<ReportSummary>>>> {
    use crate::entities::{prelude::*, tracking_reports};
    use sea_orm::*;

    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(10);

    // 验证分页参数
    if page < 1 {
        return Err(ApiError::BadRequest("Page must be >= 1".to_string()));
    }
    if !(1..=100).contains(&page_size) {
        return Err(ApiError::BadRequest(
            "Page size must be between 1 and 100".to_string(),
        ));
    }

    // 构建查询
    let mut query_builder = TrackingReports::find();

    // 应用过滤条件
    if let Some(tracking_id) = query.tracking_id {
        query_builder = query_builder.filter(tracking_reports::Column::TrackingId.eq(tracking_id));
    }
    if let Some(report_type) = query.report_type {
        query_builder = query_builder.filter(tracking_reports::Column::Source.eq(report_type));
    }
    if let Some(status) = query.status {
        query_builder = query_builder.filter(tracking_reports::Column::Status.eq(status));
    }

    // 查询总数
    let total = query_builder.clone().count(state.db.as_ref()).await?;

    // 分页查询
    let reports = query_builder
        .offset((page - 1) * page_size)
        .limit(page_size)
        .order_by_desc(tracking_reports::Column::CreatedAt)
        .find_also_related(Tracking)
        .all(state.db.as_ref())
        .await?;

    // 转换为响应格式
    let mut report_summaries = Vec::new();
    for (report, tracking_opt) in reports {
        // 获取 package 名称
        let package_name = if let Some(tracking_model) = tracking_opt {
            let package = Packages::find_by_id(tracking_model.package_id)
                .one(state.db.as_ref())
                .await?
                .map(|p| p.name)
                .unwrap_or_else(|| "unknown".to_string());
            package
        } else {
            "unknown".to_string()
        };

        report_summaries.push(ReportSummary {
            id: report.id as i64,
            tracking_id: report.tracking_id,
            report_type: report.source,
            package_name,
            status: report.status,
            created_at: report.created_at,
            updated_at: report.updated_at,
        });
    }

    let response = PaginatedResponse::new(report_summaries, total, page, page_size);
    Ok(Json(ApiResponse::success(response)))
}

/// GET /api/reports/:id
///
/// 获取报告详情
pub async fn get_report(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<ApiResponse<ReportDetail>>> {
    use crate::entities::prelude::*;
    use sea_orm::*;

    // 验证 ID
    if id <= 0 {
        return Err(ApiError::BadRequest("Invalid report ID".to_string()));
    }

