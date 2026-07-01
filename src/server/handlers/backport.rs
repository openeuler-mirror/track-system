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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Path, State};
    use sea_orm::{DatabaseBackend, MockDatabase};

    #[tokio::test]
    async fn test_list_backport_candidates_handler_empty() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<backport_candidates::Model, _, _>([vec![]])
            .into_connection();

        let state = AppState::without_external_clients(db);
        let result = list_backport_candidates_handler(Path(1), State(state)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0.len(), 0);
    }

    #[tokio::test]
    async fn test_list_backport_candidates_handler_with_data() {
        let mock_candidate = backport_candidates::Model {
            id: 1,
