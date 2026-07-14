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
