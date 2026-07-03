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

//! L1 vs L0 版本对比模块
//!
//! 用于对比发行版（L1）相对于上游社区（L0）的版本差异

use crate::utils::version::{Version, VersionParser};
use crate::utils::PatchParser;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// L0 版本信息（上游社区）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L0VersionInfo {
    /// 软件包名称
    pub package_name: String,
    /// 最新稳定版本
    pub latest_stable: String,
    /// 最新版本（可能是 beta/rc）
    pub latest_version: String,
    /// 所有版本标签
    pub all_versions: Vec<VersionTag>,
    /// 版本 changelog
    pub changelogs: HashMap<String, Vec<ChangelogEntry>>,
}

/// 版本标签
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionTag {
    /// 版本号
    pub version: String,
    /// 发布日期
    pub date: DateTime<Utc>,
    /// Changelog
    pub changelog: String,
    /// 是否为稳定版本
    pub is_stable: bool,
}

