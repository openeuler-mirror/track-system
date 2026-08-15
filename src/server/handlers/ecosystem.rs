use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use sea_orm::*;

use crate::{
    ecosystem::EcosystemService,
    entities::{ecosystem_reports, ecosystem_targets, prelude::*},
    server::{
        api::{ApiResponse, PaginatedResponse},
        dto::{
            CreateEcosystemTargetRequest, EcosystemReportListQuery, EcosystemReportResponse,
            EcosystemTargetListQuery, EcosystemTargetResponse, UpdateEcosystemTargetRequest,
        },
        error::{ApiError, ApiResult},
        state::AppState,
    },
};

pub async fn list_targets(
    State(state): State<AppState>,
    Query(query): Query<EcosystemTargetListQuery>,
) -> ApiResult<Json<ApiResponse<PaginatedResponse<EcosystemTargetResponse>>>> {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(10);
    if page < 1 || !(1..=100).contains(&page_size) {
        return Err(ApiError::BadRequest(
            "invalid pagination parameters".to_string(),
        ));
    }

    let mut builder = EcosystemTargets::find();
    if let Some(target_type) = query.target_type {
        builder = builder.filter(ecosystem_targets::Column::TargetType.eq(target_type));
    }
    if let Some(platform) = query.platform {
        builder = builder.filter(ecosystem_targets::Column::Platform.eq(platform));
    }
    if let Some(status) = query.status {
        builder = builder.filter(ecosystem_targets::Column::Status.eq(status));
    }

    let total = builder.clone().count(state.db.as_ref()).await?;
    let items = builder
        .order_by_desc(ecosystem_targets::Column::UpdatedAt)
        .offset((page - 1) * page_size)
        .limit(page_size)
        .all(state.db.as_ref())
        .await?;
    let resp = items.into_iter().map(Into::into).collect();
    Ok(Json(ApiResponse::success(PaginatedResponse::new(
        resp, total, page, page_size,
    ))))
}

pub async fn create_target(
    State(state): State<AppState>,
    Json(req): Json<CreateEcosystemTargetRequest>,
) -> ApiResult<(StatusCode, Json<ApiResponse<EcosystemTargetResponse>>)> {
    if req.name.trim().is_empty()
        || req.target_type.trim().is_empty()
        || req.role.trim().is_empty()
        || req.rule_profile.trim().is_empty()
    {
        return Err(ApiError::BadRequest(
            "name/target_type/role/rule_profile are required".to_string(),
        ));
    }

    let now = Utc::now();
    let target = ecosystem_targets::ActiveModel {
        name: Set(req.name),
        target_type: Set(req.target_type),
        platform: Set(req.platform),
        role: Set(req.role),
        homepage_url: Set(req.homepage_url),
        api_base_url: Set(req.api_base_url),
        owner: Set(req.owner),
        repo: Set(req.repo),
        default_branch: Set(req.default_branch),
        status: Set(req.status.unwrap_or_else(|| "active".to_string())),
        refresh_interval_hours: Set(req.refresh_interval_hours.unwrap_or(24)),
        rule_profile: Set(req.rule_profile),
        metadata: Set(req.metadata),
        last_collected_at: Set(None),
        last_report_at: Set(None),
        last_error: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    let inserted = target.insert(state.db.as_ref()).await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::created(inserted.into())),
    ))
}

pub async fn get_target(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<Json<ApiResponse<EcosystemTargetResponse>>> {
    let target = EcosystemTargets::find_by_id(id)
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("ecosystem target {} not found", id)))?;
    Ok(Json(ApiResponse::success(target.into())))
}

pub async fn update_target(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<UpdateEcosystemTargetRequest>,
) -> ApiResult<Json<ApiResponse<EcosystemTargetResponse>>> {
    let target = EcosystemTargets::find_by_id(id)
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("ecosystem target {} not found", id)))?;
    let mut target: ecosystem_targets::ActiveModel = target.into();

    if let Some(name) = req.name {
        target.name = Set(name);
    }
    if let Some(target_type) = req.target_type {
        target.target_type = Set(target_type);
    }
    if let Some(platform) = req.platform {
        target.platform = Set(Some(platform));
    }
    if let Some(role) = req.role {
        target.role = Set(role);
    }
    if let Some(homepage_url) = req.homepage_url {
        target.homepage_url = Set(Some(homepage_url));
    }
    if let Some(api_base_url) = req.api_base_url {
        target.api_base_url = Set(Some(api_base_url));
    }
    if let Some(owner) = req.owner {
        target.owner = Set(Some(owner));
    }
    if let Some(repo) = req.repo {
        target.repo = Set(Some(repo));
    }
    if let Some(default_branch) = req.default_branch {
        target.default_branch = Set(Some(default_branch));
    }
    if let Some(status) = req.status {
        target.status = Set(status);
    }
    if let Some(refresh_interval_hours) = req.refresh_interval_hours {
        target.refresh_interval_hours = Set(refresh_interval_hours);
    }
    if let Some(rule_profile) = req.rule_profile {
        target.rule_profile = Set(rule_profile);
    }
    if let Some(metadata) = req.metadata {
        target.metadata = Set(Some(metadata));
    }
    if let Some(last_error) = req.last_error {
        target.last_error = Set(Some(last_error));
    }
    target.updated_at = Set(Utc::now());

    let updated = target.update(state.db.as_ref()).await?;
    Ok(Json(ApiResponse::success(updated.into())))
}

pub async fn delete_target(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<StatusCode> {
    let result = EcosystemTargets::delete_by_id(id)
        .exec(state.db.as_ref())
        .await?;
    if result.rows_affected == 0 {
        return Err(ApiError::NotFound(format!(
            "ecosystem target {} not found",
            id
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn refresh_target(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<Json<ApiResponse<crate::ecosystem::EcosystemRefreshResult>>> {
    let service = EcosystemService::new(state.db.as_ref());
    let result = service
        .refresh_target(id)
        .await
        .map_err(|e| ApiError::InternalError(e.to_string()))?;
    Ok(Json(ApiResponse::success(result)))
}

pub async fn get_latest_report(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<Json<ApiResponse<EcosystemReportResponse>>> {
    let service = EcosystemService::new(state.db.as_ref());
    let report = service
        .latest_report(id)
        .await
        .map_err(|e| ApiError::InternalError(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound(format!("latest report for target {} not found", id)))?;
    Ok(Json(ApiResponse::success(report.into())))
}

pub async fn list_reports(
    State(state): State<AppState>,
    Query(query): Query<EcosystemReportListQuery>,
) -> ApiResult<Json<ApiResponse<PaginatedResponse<EcosystemReportResponse>>>> {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(10);
    if page < 1 || !(1..=100).contains(&page_size) {
        return Err(ApiError::BadRequest(
            "invalid pagination parameters".to_string(),
        ));
    }

    let mut builder = EcosystemReports::find();
    if let Some(target_id) = query.target_id {
        builder = builder.filter(ecosystem_reports::Column::TargetId.eq(target_id));
    }
    if let Some(report_type) = query.report_type {
        builder = builder.filter(ecosystem_reports::Column::ReportType.eq(report_type));
    }

    let total = builder.clone().count(state.db.as_ref()).await?;
    let items = builder
        .order_by_desc(ecosystem_reports::Column::GeneratedAt)
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
) -> ApiResult<Json<ApiResponse<EcosystemReportResponse>>> {
    let report = EcosystemReports::find_by_id(id)
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("ecosystem report {} not found", id)))?;
    Ok(Json(ApiResponse::success(report.into())))
}
