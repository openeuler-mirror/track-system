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

use crate::entities::{packages, tracking};

/// 创建软件包请求
#[derive(Debug, Deserialize)]
pub struct CreatePackageRequest {
    pub name: String,
    pub level: i32,
    pub sync_interval_hours: i32,
    pub l0_repo_url: Option<String>,
    pub description: Option<String>,
}

/// 更新软件包请求
#[derive(Debug, Deserialize)]
pub struct UpdatePackageRequest {
    pub level: Option<i32>,
    pub sync_interval_hours: Option<i32>,
    pub l0_repo_url: Option<String>,
    pub description: Option<String>,
}

