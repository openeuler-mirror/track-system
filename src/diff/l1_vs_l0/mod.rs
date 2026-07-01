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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1VsL0Report {
    /// 报告 ID
    pub id: Option<i64>,
    /// 软件包名称
    pub package_name: String,
    /// 当前版本
    pub current_version: String,
    /// 最新稳定版本
    pub latest_stable: String,
    /// 最新版本
    pub latest_version: String,
    /// 落后版本数
    pub version_behind: u32,
    /// 可升级版本列表
    pub upgradable_versions: Vec<UpgradableVersion>,
    /// 补丁分析
    pub patch_analysis: PatchAnalysis,
    /// CVE 分析
    pub cve_analysis: CveAnalysis,
    /// 升级建议
    pub recommendations: Vec<String>,
    /// 生成时间
    pub created_at: DateTime<Utc>,
}

/// 可升级版本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradableVersion {
    /// 版本号
    pub version: String,
    /// 发布日期
    pub release_date: DateTime<Utc>,
    /// 是否为安全更新
    pub is_security_release: bool,
    /// Breaking changes
    pub breaking_changes: Vec<String>,
}

/// 补丁分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchAnalysis {
    /// 总补丁数
    pub total_patches: usize,
    /// 已合并到上游的补丁
    pub merged_in_upstream: Vec<PatchInfo>,
    /// 仍需保留的补丁
    pub still_needed: Vec<PatchInfo>,
    /// 升级后可移除的补丁数
    pub can_be_removed_after_upgrade: usize,
}

/// CVE 分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CveAnalysis {
    /// 总 CVE 数
    pub total_cves: usize,
    /// 已在上游修复的 CVE
    pub fixed_in_upstream: Vec<CveInfo>,
    /// 未在上游修复的 CVE
    pub not_fixed_in_upstream: Vec<CveInfo>,
}

/// L1 vs L0 对比器
pub struct L1VsL0Comparator;

impl L1VsL0Comparator {
    /// 创建新的对比器
    pub fn new() -> Self {
        Self
    }

    /// 执行版本对比
    pub async fn compare(
        &self,
        l0_info: &L0VersionInfo,
        l1_info: &L1VersionInfo,
    ) -> Result<L1VsL0Report> {
        // 1. 版本对比
        let version_comparison = self.compare_versions(
            &l1_info.current_version,
            &l0_info.latest_stable,
            &l0_info.latest_version,
            &l0_info.all_versions,
        )?;

        // 2. 识别可升级版本
        let upgradable_versions =
            self.find_upgradable_versions(&l1_info.current_version, &l0_info.all_versions)?;

        // 3. 分析补丁状态
        let patch_analysis =
            self.analyze_patches(&l1_info.patches, &l0_info.changelogs, &upgradable_versions)?;

        // 4. CVE 分析
        let cve_analysis = self.analyze_cve_patches(&l1_info.cve_patches, &l0_info.changelogs)?;

        // 5. 生成升级建议
        let recommendations = self.generate_recommendations(
            &version_comparison,
            &patch_analysis,
            &cve_analysis,
            &upgradable_versions,
        )?;

        Ok(L1VsL0Report {
            id: None,
            package_name: l1_info.package_name.clone(),
            current_version: l1_info.current_version.clone(),
            latest_stable: l0_info.latest_stable.clone(),
            latest_version: l0_info.latest_version.clone(),
            version_behind: version_comparison.behind_count,
            upgradable_versions,
            patch_analysis,
            cve_analysis,
            recommendations,
            created_at: Utc::now(),
        })
    }

    /// 对比版本
    fn compare_versions(
        &self,
        current: &str,
        latest_stable: &str,
        latest: &str,
        all_versions: &[VersionTag],
    ) -> Result<VersionComparison> {
        // 解析当前版本
        let current_version = VersionParser::parse(current)?;

        // 解析最新稳定版本
        let latest_stable_version = VersionParser::parse(latest_stable)?;

        // 解析最新版本
        let latest_version = VersionParser::parse(latest)?;

        // 解析所有版本
        let parsed_versions: Vec<Version> = all_versions
            .iter()
            .filter_map(|tag| VersionParser::parse(&tag.version).ok())
            .collect();

        // 计算落后的版本数（只计算稳定版本）
        let behind_count = VersionParser::count_versions_behind(&current_version, &parsed_versions);

        // 判断是否过时
        let is_outdated = current_version.is_older_than(&latest_stable_version);

        // 判断是否有更新的稳定版本
        let has_newer_stable = current_version.is_older_than(&latest_stable_version);

        // 判断是否有更新的版本（包括预发布）
        let has_newer_latest = current_version.is_older_than(&latest_version);

        Ok(VersionComparison {
            behind_count,
            is_outdated,
            has_newer_stable,
            has_newer_latest,
        })
    }

    /// 查找可升级版本
    fn find_upgradable_versions(
        &self,
        current: &str,
        all_versions: &[VersionTag],
    ) -> Result<Vec<UpgradableVersion>> {
        // 解析当前版本
        let current_version = VersionParser::parse(current)?;

        // 过滤出比当前版本新的稳定版本
        let mut upgradable: Vec<UpgradableVersion> = all_versions
            .iter()
            .filter_map(|tag| {
                // 解析版本
                let version = VersionParser::parse(&tag.version).ok()?;

                // 只考虑稳定版本且比当前版本新
                if version.is_stable() && version.is_newer_than(&current_version) {
                    // 检查是否为安全更新（从 changelog 中判断）
                    let is_security_release = tag.changelog.to_lowercase().contains("security")
                        || tag.changelog.to_lowercase().contains("cve");

                    // 提取 breaking changes（简化版本：检查主版本号是否变化）
                    let breaking_changes = if version.major > current_version.major {
                        vec![format!(
                            "主版本号从 {} 升级到 {}，可能包含不兼容的变更",
                            current_version.major, version.major
                        )]
                    } else {
                        Vec::new()
                    };

                    Some(UpgradableVersion {
                        version: tag.version.clone(),
                        release_date: tag.date,
                        is_security_release,
                        breaking_changes,
                    })
                } else {
                    None
