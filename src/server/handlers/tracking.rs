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

//! 跟踪配置管理 API handlers

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::Utc;
use sea_orm::*;
use serde::{Deserialize, Serialize};

use crate::{
    entities::{prelude::*, tracking},
    server::{
        api::{ApiResponse, PaginatedResponse},
        error::{ApiError, ApiResult},
        state::AppState,
    },
};

/// 跟踪配置列表查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingListQuery {
    /// 页码（从 1 开始）
    pub page: Option<u64>,
    /// 每页大小
    pub page_size: Option<u64>,
    /// 按软件包 ID 过滤
    pub package_id: Option<i32>,
    /// 按发行版 ID 过滤
    pub distro_id: Option<i32>,
    /// 按状态过滤
    pub tracking_status: Option<String>,
}
