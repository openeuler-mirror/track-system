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

/// 变更类型定义
use serde::{Deserialize, Serialize};

/// 变更类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    /// CVE 安全漏洞修复
    CVE,
    /// Bug 修复
    Bugfix,
    /// 回合移植
    Backport,
    /// 未知类型
    Unknown,
}

impl ChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChangeType::CVE => "CVE",
            ChangeType::Bugfix => "Bugfix",
            ChangeType::Backport => "Backport",
            ChangeType::Unknown => "Unknown",
        }
    }
}

/// 补丁变更统计
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchChanges {
    /// 新增补丁数量
    pub added: usize,
    /// 删除补丁数量
    pub deleted: usize,
    /// 修改补丁数量
    pub modified: usize,
}

/// 版本信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionInfo {
    /// 旧版本
    pub old_version: Option<String>,
    /// 新版本
    pub new_version: String,
}

/// 变更分类结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeClassification {
    /// 主要变更类型
    pub primary_type: ChangeType,
    /// 影响的文件列表
    pub affected_files: Vec<String>,
    /// spec 文件变更列表
    pub spec_changes: Vec<String>,
    /// 是否有 spec 变更
    pub has_spec_change: bool,
    /// 补丁变更统计
    pub patch_changes: PatchChanges,
    /// 源码文件变更数量
    pub source_changes: usize,
    /// 是否有 changelog 更新
    pub has_changelog_update: bool,
    /// 版本信息
    pub version_info: Option<VersionInfo>,
    /// CVE 编号列表
    pub cve_numbers: Vec<String>,
}

impl Default for ChangeClassification {
    fn default() -> Self {
        Self {
            primary_type: ChangeType::Unknown,
            affected_files: Vec::new(),
            spec_changes: Vec::new(),
            has_spec_change: false,
            patch_changes: PatchChanges::default(),
            source_changes: 0,
            has_changelog_update: false,
            version_info: None,
            cve_numbers: Vec::new(),
        }
    }
}
