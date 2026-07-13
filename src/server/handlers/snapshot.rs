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

//! 快照管理 API handlers

use axum::{
    extract::{Path, Query, State},
    Json,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::server::{
    error::{ApiError, ApiResult},
    state::AppState,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSnapshotsQuery {
    pub tracking_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotListItem {
    pub id: i32,
    pub tracking_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_snapshots(
    State(state): State<AppState>,
    Query(query): Query<ListSnapshotsQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    use crate::entities::l2_snapshots::Column as L2Col;
    use crate::entities::prelude::L2Snapshots;

    let mut finder = L2Snapshots::find();
    if let Some(tid) = query.tracking_id {
        finder = finder.filter(L2Col::TrackingId.eq(tid));
    }

    let models = finder
        .order_by_desc(L2Col::CreatedAt)
        .all(state.db.as_ref())
        .await
        .map_err(ApiError::DatabaseError)?;

    let items: Vec<SnapshotListItem> = models
        .into_iter()
        .map(|m| {
            let tag = m
                .payload
                .get("tag")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            SnapshotListItem {
                id: m.id,
                tracking_id: m.tracking_id,
                tag,
                created_at: m.created_at,
            }
        })
        .collect();

    Ok(Json(serde_json::json!({ "snapshots": items })))
}

/// DELETE /api/snapshot/:id
///
pub async fn delete_snapshot(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<axum::http::StatusCode> {
    use crate::entities::prelude::L2Snapshots;

    let result = L2Snapshots::delete_by_id(id)
        .exec(state.db.as_ref())
        .await
        .map_err(ApiError::DatabaseError)?;

    if result.rows_affected == 0 {
        return Err(ApiError::NotFound(format!("快照 {} 不存在", id)));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::l2_snapshots;
    use axum::extract::Query;
    use sea_orm::{DatabaseBackend, MockDatabase};

    #[tokio::test]
    async fn test_list_snapshots_all() {
        let mock_snapshot = l2_snapshots::Model {
            id: 1,
            tracking_id: 10,
            snapshot_type: "L2".to_string(),
            checksum: "abc123".to_string(),
            payload: serde_json::json!({
                "tag": "v1.0.0",
                "files": []
            }),
            created_at: chrono::Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[mock_snapshot.clone()]])
            .into_connection();

        let state = AppState::without_external_clients(db);
        let query = ListSnapshotsQuery { tracking_id: None };

        let result = list_snapshots(State(state), Query(query)).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        let snapshots = response.0["snapshots"].as_array().unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0]["id"], 1);
        assert_eq!(snapshots[0]["tracking_id"], 10);
        assert_eq!(snapshots[0]["tag"], "v1.0.0");
    }

    #[tokio::test]
    async fn test_list_snapshots_filtered_by_tracking_id() {
        let mock_snapshot1 = l2_snapshots::Model {
            id: 1,
            tracking_id: 10,
            snapshot_type: "L2".to_string(),
            checksum: "abc123".to_string(),
            payload: serde_json::json!({"tag": "v1.0.0"}),
            created_at: chrono::Utc::now(),
        };

        let mock_snapshot2 = l2_snapshots::Model {
            id: 2,
            tracking_id: 10,
            snapshot_type: "L2".to_string(),
            checksum: "abc123".to_string(),
            payload: serde_json::json!({}),
            created_at: chrono::Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[mock_snapshot1, mock_snapshot2]])
            .into_connection();

        let state = AppState::without_external_clients(db);
        let query = ListSnapshotsQuery {
            tracking_id: Some(10),
        };

        let result = list_snapshots(State(state), Query(query)).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        let snapshots = response.0["snapshots"].as_array().unwrap();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0]["tracking_id"], 10);
        assert_eq!(snapshots[1]["tracking_id"], 10);
    }

    #[tokio::test]
    async fn test_list_snapshots_empty() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<l2_snapshots::Model, _, _>([vec![]])
            .into_connection();

        let state = AppState::without_external_clients(db);
        let query = ListSnapshotsQuery {
            tracking_id: Some(999),
        };

        let result = list_snapshots(State(state), Query(query)).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        let snapshots = response.0["snapshots"].as_array().unwrap();
        assert_eq!(snapshots.len(), 0);
    }

    #[tokio::test]
    async fn test_list_snapshots_without_tag() {
        let mock_snapshot = l2_snapshots::Model {
            id: 3,
            tracking_id: 20,
            snapshot_type: "L2".to_string(),
            checksum: "abc123".to_string(),
            payload: serde_json::json!({"other_field": "value"}),
            created_at: chrono::Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[mock_snapshot]])
            .into_connection();

        let state = AppState::without_external_clients(db);
        let query = ListSnapshotsQuery { tracking_id: None };

        let result = list_snapshots(State(state), Query(query)).await;
        assert!(result.is_ok());
