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
