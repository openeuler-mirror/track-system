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

/// Changelog 条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    /// 类型（feature, bugfix, security, etc.）
    pub entry_type: String,
    /// 描述
    pub description: String,
    /// 相关的 commit SHA
    pub commit_sha: Option<String>,
}

/// L1 版本信息（发行版）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1VersionInfo {
    /// 软件包名称
    pub package_name: String,
    /// 当前版本（从 spec 文件提取）
    pub current_version: String,
    /// Patch 列表
    pub patches: Vec<PatchInfo>,
    /// CVE 补丁
    pub cve_patches: Vec<CveInfo>,
}

/// Patch 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchInfo {
    /// 文件名
    pub filename: String,
    /// 描述
    pub description: String,
    /// 是否已应用
    pub applied: bool,
    /// 内容哈希
    pub content_hash: Option<String>,
}

/// CVE 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CveInfo {
    /// CVE 编号
    pub cve_id: String,
    /// 补丁文件
    pub patch_file: String,
    /// 描述
    pub description: String,
    /// 严重程度
    pub severity: Option<String>,
}

/// L1 vs L0 对比报告
