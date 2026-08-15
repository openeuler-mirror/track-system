use axum::{
    extract::{Path, Query, State},
    Json,
};
use sea_orm::*;

use crate::{
    ecosystem::maintenance::MaintenanceService,
    entities::{maintenance_reports, prelude::*},
    server::{
        api::{ApiResponse, PaginatedResponse},
        dto::{MaintenanceReportListQuery, MaintenanceReportResponse},
        error::{ApiError, ApiResult},
        state::AppState,
    },
};

pub async fn refresh_package(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<Json<ApiResponse<crate::ecosystem::maintenance::MaintenanceRefreshResult>>> {
    let service = MaintenanceService::new(state.db.as_ref());
    let result = service
        .refresh_package(id)
        .await
        .map_err(|e| ApiError::InternalError(e.to_string()))?;
    Ok(Json(ApiResponse::success(result)))
}

pub async fn get_latest_report(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<Json<ApiResponse<MaintenanceReportResponse>>> {
    let service = MaintenanceService::new(state.db.as_ref());
    let report = service
        .latest_report(id)
        .await
        .map_err(|e| ApiError::InternalError(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound(format!("latest report for package {} not found", id)))?;
    Ok(Json(ApiResponse::success(report.into())))
}

pub async fn list_reports(
    State(state): State<AppState>,
    Query(query): Query<MaintenanceReportListQuery>,
) -> ApiResult<Json<ApiResponse<PaginatedResponse<MaintenanceReportResponse>>>> {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(10);
    if page < 1 || !(1..=100).contains(&page_size) {
        return Err(ApiError::BadRequest(
            "invalid pagination parameters".to_string(),
        ));
    }

    let mut builder = MaintenanceReports::find();
    if let Some(package_id) = query.package_id {
        builder = builder.filter(maintenance_reports::Column::PackageId.eq(package_id));
    }
    if let Some(report_type) = query.report_type {
        builder = builder.filter(maintenance_reports::Column::ReportType.eq(report_type));
    }

    let total = builder.clone().count(state.db.as_ref()).await?;
    let items = builder
        .order_by_desc(maintenance_reports::Column::GeneratedAt)
        .offset((page - 1) * page_size)
        .limit(page_size)
        .all(state.db.as_ref())
        .await?;
    let resp = items.into_iter().map(Into::into).collect();
    Ok(Json(ApiResponse::success(PaginatedResponse::new(
        resp, total, page, page_size,
    ))))
}

pub async fn get_report(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<ApiResponse<MaintenanceReportResponse>>> {
    let report = MaintenanceReports::find_by_id(id)
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("maintenance report {} not found", id)))?;
    Ok(Json(ApiResponse::success(report.into())))
}
