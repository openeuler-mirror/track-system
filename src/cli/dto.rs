/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
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
use serde_json::Value;

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
    pub package_name: Option<String>,
    pub package_level: Option<i32>,
    pub l0_repo_url: Option<String>,
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
    pub maintenance_summary: Option<TrackingMaintenanceSummaryDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingMaintenanceSummaryDto {
    pub report_id: i64,
    pub overall_risk: String,
    pub confidence: String,
    pub generated_at: DateTime<Utc>,
    pub commit_total: Option<i64>,
    pub commits_last_12_months: Option<i64>,
    pub committers_last_12_months: Option<i64>,
    pub last_commit_at: Option<String>,
    pub stars: Option<i64>,
    pub forks: Option<i64>,
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
pub struct CreateDistroRequest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
}

/// 更新发行版请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDistroRequest {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
}

/// 创建跟踪配置请求（与服务端一致）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTrackingRequest {
    pub package_id: i32,
    pub distro_id: i32,
    pub l1_repo_owner: String,
    pub l1_repo_name: String,
    pub l1_branch: String,
    pub l2_branch: String,
    pub l2_repo_path: String,
    pub tracking_status: Option<String>,
}

/// 更新跟踪配置请求（与服务端一致）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTrackingRequest {
    pub l1_repo_owner: Option<String>,
    pub l1_repo_name: Option<String>,
    pub l1_branch: Option<String>,
    pub l2_branch: Option<String>,
    pub l2_repo_path: Option<String>,
    pub tracking_status: Option<String>,
}

/// 生态目标 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemTargetDto {
    pub id: i32,
    pub name: String,
    pub target_type: String,
    pub platform: Option<String>,
    pub role: String,
    pub homepage_url: Option<String>,
    pub api_base_url: Option<String>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub default_branch: Option<String>,
    pub status: String,
    pub refresh_interval_hours: i32,
    pub rule_profile: String,
    pub metadata: Option<Value>,
    pub last_collected_at: Option<DateTime<Utc>>,
    pub last_report_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 生态报告 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemReportDto {
    pub id: i64,
    pub target_id: i32,
    pub report_type: String,
    pub status: String,
    pub overall_risk: String,
    pub confidence: String,
    pub summary: String,
    pub dimensions: Value,
    pub evidence_summary: Option<Value>,
    pub report_payload: Value,
    pub generated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 生态目标创建请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEcosystemTargetRequest {
    pub name: String,
    pub target_type: String,
    pub platform: Option<String>,
    pub role: String,
    pub homepage_url: Option<String>,
    pub api_base_url: Option<String>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub default_branch: Option<String>,
    pub status: Option<String>,
    pub refresh_interval_hours: Option<i32>,
    pub rule_profile: String,
    pub metadata: Option<Value>,
}

/// 生态目标更新请求
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateEcosystemTargetRequest {
    pub name: Option<String>,
    pub target_type: Option<String>,
    pub platform: Option<String>,
    pub role: Option<String>,
    pub homepage_url: Option<String>,
    pub api_base_url: Option<String>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub default_branch: Option<String>,
    pub status: Option<String>,
    pub refresh_interval_hours: Option<i32>,
    pub rule_profile: Option<String>,
    pub metadata: Option<Value>,
    pub last_error: Option<String>,
}

/// 生态目标刷新结果 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemRefreshResultDto {
    pub target_id: i32,
    pub evidence_count: usize,
    pub report_id: i64,
    pub generated_at: DateTime<Utc>,
}

/// 维护评估报告 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceReportDto {
    pub id: i64,
    pub package_id: i32,
    pub report_type: String,
    pub status: String,
    pub overall_risk: String,
    pub confidence: String,
    pub summary: String,
    pub dimensions: Value,
    pub evidence_summary: Option<Value>,
    pub report_payload: Value,
    pub generated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 维护评估目标刷新结果 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceRefreshResultDto {
    pub package_id: i32,
    pub evidence_count: usize,
    pub report_id: i64,
    pub generated_at: DateTime<Utc>,
}
