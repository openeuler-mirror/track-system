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
