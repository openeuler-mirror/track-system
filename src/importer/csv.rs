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

//! CSV 软件包导入器
//!
//! 从 CSV 文件批量导入软件包配置

use chrono::Utc;
use sea_orm::*;
use serde::Deserialize;
use std::path::Path;

use crate::entities::{packages, prelude::*};

/// CSV 记录结构
#[derive(Debug, Deserialize)]
pub struct PackageRecord {
    pub name: String,
    pub level: i32,
    #[serde(default)]
    pub sync_interval_hours: Option<i32>,
    #[serde(default)]
    pub l0_repo_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// 导入结果
#[derive(Debug)]
pub struct ImportResult {
    pub success: bool,
    pub stats: ImportStats,
    pub errors: Vec<String>,
