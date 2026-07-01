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
                }
            })
            .collect();

        // 按版本号排序（从旧到新）
        upgradable.sort_by(|a, b| {
            let va = VersionParser::parse(&a.version).unwrap_or_else(|_| Version::new(0, 0, 0));
            let vb = VersionParser::parse(&b.version).unwrap_or_else(|_| Version::new(0, 0, 0));
            va.cmp(&vb)
        });

        Ok(upgradable)
    }

    /// 分析补丁状态
    ///
    /// 通过以下策略判断补丁是否已合并到上游：
    /// 1. 检查补丁是否标记为 backport（文件名或内容包含 backport/upstream/cherry-pick）
    /// 2. 如果是 backport，提取上游 commit SHA，检查是否在 changelog 中
    /// 3. 对于非 backport 补丁，通过描述关键词匹配 changelog 条目
    /// 4. 检查补丁修复的问题是否在可升级版本的 changelog 中提及
    fn analyze_patches(
        &self,
        patches: &[PatchInfo],
        changelogs: &HashMap<String, Vec<ChangelogEntry>>,
        upgradable_versions: &[UpgradableVersion],
    ) -> Result<PatchAnalysis> {
        let mut merged_in_upstream = Vec::new();
        let mut still_needed = Vec::new();

        for patch in patches {
            // 解析补丁内容（如果有内容哈希，说明已经解析过）
            let is_merged = if let Some(content_hash) = &patch.content_hash {
                // 使用内容哈希判断（简化版本：假设有哈希就是已解析）
                self.is_patch_merged_in_upstream(
                    patch,
                    content_hash,
                    changelogs,
                    upgradable_versions,
                )?
            } else {
                // 没有内容哈希，保守判断为仍需保留
                false
            };

            if is_merged {
                merged_in_upstream.push(patch.clone());
            } else {
                still_needed.push(patch.clone());
            }
        }

        // 计算升级后可移除的补丁数
        let can_be_removed_after_upgrade = merged_in_upstream.len();

        Ok(PatchAnalysis {
            total_patches: patches.len(),
            merged_in_upstream,
            still_needed,
            can_be_removed_after_upgrade,
        })
    }

    /// 判断补丁是否已合并到上游
    fn is_patch_merged_in_upstream(
        &self,
        patch: &PatchInfo,
        _content_hash: &str,
        changelogs: &HashMap<String, Vec<ChangelogEntry>>,
        upgradable_versions: &[UpgradableVersion],
    ) -> Result<bool> {
        // 策略 1: 检查是否为 backport patch
        if PatchParser::is_backport_patch(&patch.filename, &patch.description) {
            // 如果是 backport，尝试提取上游 commit SHA
            if let Some(upstream_commit) = PatchParser::extract_upstream_commit(&patch.description)
            {
                // 检查这个 commit 是否在任何 changelog 中
                for changelog_entries in changelogs.values() {
                    for entry in changelog_entries {
                        if let Some(commit_sha) = &entry.commit_sha {
                            // 支持短 SHA 匹配（前 7 位）
                            if commit_sha
                                .starts_with(&upstream_commit[..7.min(upstream_commit.len())])
                                || upstream_commit
                                    .starts_with(&commit_sha[..7.min(commit_sha.len())])
                            {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }

        // 策略 2: 通过描述关键词匹配
        if !patch.description.is_empty() {
            // 提取补丁描述的关键词（简化版本：使用前 50 个字符）
            let patch_keywords = patch
                .description
                .to_lowercase()
                .chars()
                .take(50)
                .collect::<String>();

            // 检查可升级版本的 changelog
            for version in upgradable_versions {
                if let Some(changelog_entries) = changelogs.get(&version.version) {
                    for entry in changelog_entries {
                        let entry_desc = entry.description.to_lowercase();

                        // 如果 changelog 条目包含补丁的关键词，认为可能已合并
                        if !patch_keywords.is_empty()
                            && entry_desc.contains(&patch_keywords[..20.min(patch_keywords.len())])
                        {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        // 策略 3: 检查 CVE 补丁
        let cve_ids = PatchParser::extract_cve_from_filename(&patch.filename);
        if !cve_ids.is_empty() {
            // 检查这些 CVE 是否在上游的 changelog 中提及
            for version in upgradable_versions {
                if let Some(changelog_entries) = changelogs.get(&version.version) {
                    for entry in changelog_entries {
                        let entry_desc = entry.description.to_lowercase();

                        // 检查是否提及相同的 CVE
                        for cve_id in &cve_ids {
                            if entry_desc.contains(&cve_id.to_lowercase()) {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }

        // 默认：保守判断为仍需保留
        Ok(false)
    }

    /// 分析 CVE 补丁
    ///
    /// 通过以下策略判断 CVE 是否已在上游修复：
    /// 1. 检查 changelog 条目中是否直接提及 CVE 编号
    /// 2. 检查 changelog 条目的类型是否为 security
    /// 3. 检查 changelog 条目的描述是否包含 CVE 相关关键词
    fn analyze_cve_patches(
        &self,
        cve_patches: &[CveInfo],
        changelogs: &HashMap<String, Vec<ChangelogEntry>>,
    ) -> Result<CveAnalysis> {
        let mut fixed_in_upstream = Vec::new();
        let mut not_fixed_in_upstream = Vec::new();

        for cve in cve_patches {
            let is_fixed = self.is_cve_fixed_in_upstream(&cve.cve_id, changelogs)?;

            if is_fixed {
                fixed_in_upstream.push(cve.clone());
            } else {
                not_fixed_in_upstream.push(cve.clone());
            }
        }

        Ok(CveAnalysis {
            total_cves: cve_patches.len(),
            fixed_in_upstream,
            not_fixed_in_upstream,
        })
    }

    /// 判断 CVE 是否已在上游修复
    ///
    /// 策略：
    /// 1. 直接匹配：检查 changelog 条目中是否包含 CVE 编号
    /// 2. 模糊匹配：检查 security 类型的 changelog 条目
    fn is_cve_fixed_in_upstream(
        &self,
        cve_id: &str,
        changelogs: &HashMap<String, Vec<ChangelogEntry>>,
    ) -> Result<bool> {
        let cve_id_lower = cve_id.to_lowercase();

        // 遍历所有版本的 changelog
        for changelog_entries in changelogs.values() {
            for entry in changelog_entries {
                // 策略 1: 直接匹配 CVE 编号
                let entry_desc_lower = entry.description.to_lowercase();
                if entry_desc_lower.contains(&cve_id_lower) {
                    return Ok(true);
                }

                // 策略 2: 检查 security 类型的条目
                // 如果是 security 类型，且描述中包含相关关键词，可能是相关修复
                if entry.entry_type.to_lowercase() == "security" {
                    // 提取 CVE 年份和编号（例如 CVE-2023-1234 -> 2023, 1234）
                    if let Some((year, number)) = Self::parse_cve_id(cve_id) {
                        // 检查 changelog 中是否提及相同年份的 CVE
