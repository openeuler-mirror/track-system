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
