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

//! CLI 数据传输对象（DTO）
//!
//! 定义客户端专用的数据结构，不依赖数据库 entities

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 软件包信息 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDto {
    pub id: i32,
    pub name: String,
    pub level: i32,
    pub sync_interval_hours: i32,
    pub l0_repo_url: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 发行版信息 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistroDto {
    pub id: i32,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 跟踪配置 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingDto {
    pub id: i32,
    pub package_id: i32,
    pub distro_id: i32,
    pub l1_repo_owner: String,
    pub l1_repo_name: String,
    pub l1_branch: String,
    pub l2_branch: String,
    pub l2_repo_path: String,
    pub tracking_status: String,
    pub last_sync_time: Option<DateTime<Utc>>,
    pub last_l1_commit_sha: Option<String>,
    pub last_l2_commit_sha: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// L2 快照信息 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2SnapshotDto {
    pub id: i32,
    pub tracking_id: i32,
    pub commit_hash: String,
    pub commit_message: String,
    pub commit_author: String,
    pub commit_date: DateTime<Utc>,
    pub spec_version: Option<String>,
    pub spec_release: Option<String>,
    pub snapshot_data: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// 同步状态 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatusDto {
    pub tracking_id: i32,
    pub package_name: String,
    pub distro_name: String,
    pub status: String,
    pub last_sync: Option<DateTime<Utc>>,
    pub next_sync: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

/// 创建软件包请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePackageRequest {
    pub name: String,
    pub level: i32,
    pub sync_interval_hours: i32,
    pub l0_repo_url: Option<String>,
    pub description: Option<String>,
}
/// 更新软件包请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePackageRequest {
    pub level: Option<i32>,
    pub sync_interval_hours: Option<i32>,
    pub l0_repo_url: Option<String>,
    pub description: Option<String>,
}

/// 创建发行版请求
#[derive(Debug, Clone, Serialize, Deserialize)]
