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

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 组件请求体
#[derive(Debug, Deserialize)]
pub struct ComponentRequest {
    pub name: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub spec: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
}

/// 批量查询请求
#[derive(Debug, Deserialize)]
pub struct ComponentQueryRequest {
    pub components: Vec<ComponentRequest>,
}

/// 组件查询参数
#[derive(Debug, Deserialize)]
pub struct ComponentQueryParams {
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub spec: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
}

/// 组件信息响应
#[derive(Debug, Serialize)]
pub struct ComponentInfo {
    pub name: String,
    pub version: String,
    pub release: String,
}

/// commit 查询参数
#[derive(Debug, Deserialize)]
pub struct ComponentCommitParams {
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
    #[serde(default)]
    pub platform: Option<String>,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

/// 组件 commit 响应
#[derive(Debug, Serialize)]
pub struct ComponentCommitDto {
    pub sha: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: DateTime<Utc>,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additions: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deletions: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
}
