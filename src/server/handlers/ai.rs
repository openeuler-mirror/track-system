/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FITNESS FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

use axum::{
    extract::{Path, State},
    Json,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;

use crate::{
    ai::{AiAnalysisService, AiAnalysisSource, AiContext},
    entities::{ecosystem_targets, packages, prelude::*},
    server::{
        api::ApiResponse,
        dto::AiAnalyzeRequest,
        error::{ApiError, ApiResult},
        state::AppState,
    },
};

pub async fn analyze_ecosystem_report(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<AiAnalyzeRequest>,
) -> ApiResult<Json<ApiResponse<crate::ai::AiAnalysisResponse>>> {
    let report = EcosystemReports::find_by_id(id)
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("ecosystem report {} not found", id)))?;
    let target = EcosystemTargets::find()
        .filter(ecosystem_targets::Column::Id.eq(report.target_id))
        .one(state.db.as_ref())
        .await?;

    let context = AiContext {
        source: AiAnalysisSource::EcosystemReport,
        target_name: target.as_ref().map(|item| item.name.clone()),
        target_type: target.as_ref().map(|item| item.target_type.clone()),
        platform: target.as_ref().and_then(|item| item.platform.clone()),
        report_type: Some(report.report_type.clone()),
        rule_risk: Some(report.overall_risk.clone()),
        rule_confidence: Some(report.confidence.clone()),
        rule_summary: Some(report.summary.clone()),
        evidence: json!({
            "dimensions": report.dimensions,
            "evidence_summary": report.evidence_summary,
            "report_payload": report.report_payload,
        }),
    };

    let service = AiAnalysisService::from_env();
    let response = service
        .analyze(context, req.into())
        .await
        .map_err(|err| ApiError::InternalError(err.to_string()))?;
    Ok(Json(ApiResponse::success(response)))
}

pub async fn analyze_maintenance_report(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<AiAnalyzeRequest>,
) -> ApiResult<Json<ApiResponse<crate::ai::AiAnalysisResponse>>> {
    let report = MaintenanceReports::find_by_id(id)
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("maintenance report {} not found", id)))?;
    let package = Packages::find()
        .filter(packages::Column::Id.eq(report.package_id))
        .one(state.db.as_ref())
        .await?;

    let context = AiContext {
        source: AiAnalysisSource::MaintenanceReport,
        target_name: package.as_ref().map(|item| item.name.clone()),
        target_type: Some("package".to_string()),
        platform: None,
        report_type: Some(report.report_type.clone()),
        rule_risk: Some(report.overall_risk.clone()),
        rule_confidence: Some(report.confidence.clone()),
        rule_summary: Some(report.summary.clone()),
        evidence: json!({
            "dimensions": report.dimensions,
            "evidence_summary": report.evidence_summary,
            "report_payload": report.report_payload,
        }),
    };

    let service = AiAnalysisService::from_env();
    let response = service
        .analyze(context, req.into())
        .await
        .map_err(|err| ApiError::InternalError(err.to_string()))?;
    Ok(Json(ApiResponse::success(response)))
}
