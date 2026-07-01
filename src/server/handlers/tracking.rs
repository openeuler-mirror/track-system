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

/// 创建跟踪配置请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTrackingRequest {
    /// 软件包 ID
    pub package_id: i32,
    /// 发行版 ID
    pub distro_id: i32,
    /// L1 仓库所有者
    pub l1_repo_owner: String,
    /// L1 仓库名称
    pub l1_repo_name: String,
    /// L1 分支
    pub l1_branch: String,
    /// L2 分支
    pub l2_branch: String,
    /// L2 仓库路径
    pub l2_repo_path: String,
    /// 跟踪状态
    pub tracking_status: Option<String>,
}

/// 更新跟踪配置请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTrackingRequest {
    /// L1 仓库所有者
    pub l1_repo_owner: Option<String>,
    /// L1 仓库名称
    pub l1_repo_name: Option<String>,
    /// L1 分支
    pub l1_branch: Option<String>,
    /// L2 分支
    pub l2_branch: Option<String>,
    /// L2 仓库路径
    pub l2_repo_path: Option<String>,
    /// 跟踪状态
    pub tracking_status: Option<String>,
}

/// 跟踪配置响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingResponse {
    pub id: i32,
    pub package_id: i32,
    pub distro_id: i32,
