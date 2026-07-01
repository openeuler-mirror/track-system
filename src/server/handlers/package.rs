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

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use sea_orm::*;

use crate::{
    entities::{packages, prelude::*},
    server::{
        dto::{
            CreatePackageRequest, PackageResponse, PackageWithTrackingResponse, TrackingResponse,
            UpdatePackageRequest,
        },
        error::{ApiError, ApiResult},
        state::AppState,
    },
};

/// 列出所有软件包
pub async fn list_packages(State(state): State<AppState>) -> ApiResult<Json<Vec<PackageResponse>>> {
    let packages = Packages::find().all(state.db.as_ref()).await?;

    let responses: Vec<PackageResponse> = packages.into_iter().map(Into::into).collect();

    Ok(Json(responses))
}

/// 创建软件包
pub async fn create_package(
    State(state): State<AppState>,
    Json(req): Json<CreatePackageRequest>,
) -> ApiResult<(StatusCode, Json<PackageResponse>)> {
    let now = Utc::now();

    if req.sync_interval_hours <= 0 || req.sync_interval_hours > 24 * 365 {
        return Err(ApiError::BadRequest(
            "sync_interval_hours 必须在 1..=8760 小时范围内".to_string(),
        ));
    }

    let package = packages::ActiveModel {
        name: Set(req.name),
        level: Set(req.level),
        sync_interval_hours: Set(req.sync_interval_hours),
        l0_repo_url: Set(req.l0_repo_url),
        description: Set(req.description),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };

    let result = package.insert(state.db.as_ref()).await?;

    Ok((StatusCode::CREATED, Json(result.into())))
}

/// 获取单个软件包
pub async fn get_package(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<Json<PackageResponse>> {
    let package = Packages::find_by_id(id)
        .one(state.db.as_ref())
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Package with id {} not found", id)))?;

    Ok(Json(package.into()))
