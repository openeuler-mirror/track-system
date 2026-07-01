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
