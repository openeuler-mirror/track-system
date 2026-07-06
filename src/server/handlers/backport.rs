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
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::entities::backport_candidates;
use crate::server::{dto::backport::BackportCandidateDto, state::AppState};

pub async fn list_backport_candidates_handler(
    Path(package_id): Path<i32>,
    State(state): State<AppState>,
) -> Result<Json<Vec<BackportCandidateDto>>, StatusCode> {
    let db: &DatabaseConnection = &state.db;

    let candidates = backport_candidates::Entity::find()
        .filter(backport_candidates::Column::PackageId.eq(package_id))
        .all(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(
        candidates
            .into_iter()
            .map(BackportCandidateDto::from)
            .collect(),
    ))
}

