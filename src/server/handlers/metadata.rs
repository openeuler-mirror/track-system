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

//! 元数据管理 API handlers

use axum::{
    extract::{Path, Query, State},
    Json,
};
use sea_orm::ActiveModelTrait;
use serde::{Deserialize, Serialize};

use crate::{
    server::{
        api::ApiResponse,
        error::{ApiError, ApiResult},
        state::AppState,
    },
    snapshot::types::RepositorySnapshot,
};

/// L0 元数据导入请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportL0Request {
    /// 跟踪配置 ID
    pub tracking_id: i32,
    /// 仓库快照数据
    pub snapshot: RepositorySnapshot,
}

/// L1 元数据导入请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportL1Request {
    /// 跟踪配置 ID
    pub tracking_id: i32,
    /// 仓库快照数据
    pub snapshot: RepositorySnapshot,
}

/// L2 元数据导入请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportL2Request {
    /// 跟踪配置 ID
