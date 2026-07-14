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

//! L1 vs L0 版本对比模块
//!
//! 用于对比发行版（L1）相对于上游社区（L0）的版本差异

use crate::utils::version::{Version, VersionParser};
use crate::utils::PatchParser;
use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const OUTDATED_MAJOR_VERSION_THRESHOLD: u32 = 3;

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
    /// 从 L0 仓库或组件原生社区识别出的停维/生命周期公告
    pub maintenance_notices: Vec<MaintenanceNotice>,
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
    /// L1 当前版本（从 spec 文件提取）
    pub current_version: String,
    /// 当前组件版本（L2 spec 文件版本）
    pub component_version: Option<String>,
    /// L1 仓库中可识别到的最新版本
    pub latest_version: Option<String>,
    /// L1 仓库中可识别到的版本集合
    pub known_versions: Vec<String>,
    /// 是否识别为 LTS/长期维护版本
    pub is_lts: Option<bool>,
    /// LTS 判定证据
    pub lts_evidence: Vec<String>,
    /// Patch 列表
    pub patches: Vec<PatchInfo>,
    /// CVE 补丁
    pub cve_patches: Vec<CveInfo>,
}

/// 停维/生命周期公告证据
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MaintenanceNotice {
    /// 公告中提到的版本号
    pub version: Option<String>,
    /// 归一化后的版本系列，例如 1.0
    pub series: Option<String>,
    /// 维护截止日期，采用 YYYY-MM-DD；无法精确解析时为空
    pub support_until: Option<String>,
    /// 识别出的状态，例如 OUT_OF_SUPPORT、SCHEDULED_EOL
    pub status: String,
    /// 证据来源，例如 l0_commit:sha 或 official_page:url
    pub source: String,
    /// 命中的公告片段
    pub evidence: String,
}

/// 当前版本停维状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MaintenanceStatus {
    /// SUPPORTED、SCHEDULED_EOL、OUT_OF_SUPPORT、END_OF_MAINTENANCE_NOTICE、UNKNOWN
    pub status: String,
    /// 是否识别到停维/停止支持类信息
    pub stop_maintenance_detected: bool,
    /// 与当前版本最匹配的公告
    pub matched_notice: Option<MaintenanceNotice>,
    /// 候选公告证据
    pub evidence: Vec<MaintenanceNotice>,
    /// HIGH、MEDIUM、LOW
    pub confidence: String,
}

/// 过时版本判定
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutdatedVersionAssessment {
    /// 当前组件版本（L2）
    pub current_version: String,
    /// 最新版本（L1）
    pub latest_version: Option<String>,
    pub latest_version_source: Option<String>,
    /// 主线版本（L0），仅用于展示参考，不参与过时判定
    pub mainline_version: Option<String>,
    pub mainline_version_source: Option<String>,
    pub major_version_gap: Option<u32>,
    pub threshold_major_versions: u32,
    pub is_outdated: bool,
}

/// LTS 判定
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LtsAssessment {
    pub is_lts: Option<bool>,
    pub source: String,
    pub evidence: Vec<String>,
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
    /// 当前组件版本（L2）
    pub current_version: String,
    /// 最新稳定版本（兼容字段，当前表示 L1 最新版本）
    pub latest_stable: String,
    /// 最新版本（L1）
    pub latest_version: String,
    /// 主线版本（L0），仅用于展示参考
    pub mainline_version: Option<String>,
    /// 落后版本数
    pub version_behind: u32,
    /// 可升级版本列表
    pub upgradable_versions: Vec<UpgradableVersion>,
    /// 补丁分析
    pub patch_analysis: PatchAnalysis,
    /// CVE 分析
    pub cve_analysis: CveAnalysis,
    /// L0/原生社区停维公告识别结果
    pub maintenance_status: MaintenanceStatus,
    /// 基于 L1 仓库版本信息的 3 个大版本差距判定
    pub outdated_version: OutdatedVersionAssessment,
    /// 基于 L1 仓库信息的 LTS 判定
    pub lts: LtsAssessment,
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
        let component_version = resolved_component_version(l1_info);
        let l1_latest_version = resolved_l1_latest_version(l1_info);

        // 1. 版本对比：当前组件版本来自 L2，最新版本来自 L1；L0 主线版本仅展示
        let version_comparison = self.compare_component_to_l1(l1_info)?;

        // 2. 识别可升级版本
        let upgradable_versions =
            self.find_upgradable_versions(&l1_info.current_version, &l0_info.all_versions)?;

        // 3. 分析补丁状态
        let patch_analysis =
            self.analyze_patches(&l1_info.patches, &l0_info.changelogs, &upgradable_versions)?;

        // 4. CVE 分析
        let cve_analysis = self.analyze_cve_patches(&l1_info.cve_patches, &l0_info.changelogs)?;

        // 5. 生成升级建议
        let maintenance_status =
            self.assess_maintenance_status(&component_version, &l0_info.maintenance_notices);
        let outdated_version =
            self.assess_outdated_version(l1_info, Some(l0_info.latest_version.clone()));
        let lts = self.assess_lts(l1_info);
        let recommendations = self.generate_recommendations(
            &version_comparison,
            &patch_analysis,
            &cve_analysis,
            &upgradable_versions,
            &maintenance_status,
            &outdated_version,
            &lts,
        )?;

        Ok(L1VsL0Report {
            id: None,
            package_name: l1_info.package_name.clone(),
            current_version: component_version,
            latest_stable: l1_latest_version.clone(),
            latest_version: l1_latest_version,
            mainline_version: Some(l0_info.latest_version.clone()),
            version_behind: version_comparison.behind_count,
            upgradable_versions,
            patch_analysis,
            cve_analysis,
            maintenance_status,
            outdated_version,
            lts,
            recommendations,
            created_at: Utc::now(),
        })
    }

    /// 在缺少 L0 版本/生命周期证据时生成部分 L1 vs L0 报告。
    ///
    /// LTS 仍可基于 L1 信息判断；过时版本可基于 L2 当前组件版本与
    /// L1 最新版本判断。停维生命周期依赖 L0/原生社区公告，缺失时保持
    /// UNKNOWN，避免把证据不足误判为安全。
    pub fn compare_without_l0(&self, l1_info: &L1VersionInfo) -> L1VsL0Report {
        let component_version = resolved_component_version(l1_info);
        let l1_latest_version = resolved_l1_latest_version(l1_info);
        let version_behind = self
            .compare_component_to_l1(l1_info)
            .map(|comparison| comparison.behind_count)
            .unwrap_or(0);
        let outdated_version = self.assess_outdated_version(l1_info, None);
        let lts = self.assess_lts(l1_info);
        let maintenance_status = MaintenanceStatus {
            status: "UNKNOWN".to_string(),
            stop_maintenance_detected: false,
            matched_notice: None,
            evidence: vec![],
            confidence: "LOW".to_string(),
        };
        let recommendations =
            self.generate_partial_recommendations(&maintenance_status, &outdated_version, &lts);

        L1VsL0Report {
            id: None,
            package_name: l1_info.package_name.clone(),
            current_version: component_version,
            latest_stable: l1_latest_version.clone(),
            latest_version: l1_latest_version,
            mainline_version: None,
            version_behind,
            upgradable_versions: vec![],
            patch_analysis: PatchAnalysis {
                total_patches: l1_info.patches.len(),
                merged_in_upstream: vec![],
                still_needed: l1_info.patches.clone(),
                can_be_removed_after_upgrade: 0,
            },
            cve_analysis: CveAnalysis {
                total_cves: l1_info.cve_patches.len(),
                fixed_in_upstream: vec![],
                not_fixed_in_upstream: l1_info.cve_patches.clone(),
            },
            maintenance_status,
            outdated_version,
            lts,
            recommendations,
            created_at: Utc::now(),
        }
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

        // 解析最新稳定版本；生命周期场景可能只有停维公告而没有版本标签，
        // 此时退回当前版本，保证停维/LTS/过时判定仍可生成报告。
        let latest_stable_version =
            VersionParser::parse(latest_stable).unwrap_or_else(|_| current_version.clone());

        // 解析最新版本
        let latest_version =
            VersionParser::parse(latest).unwrap_or_else(|_| latest_stable_version.clone());

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

    fn compare_component_to_l1(&self, l1_info: &L1VersionInfo) -> Result<VersionComparison> {
        let component_version = resolved_component_version(l1_info);
        let l1_latest_version = resolved_l1_latest_version(l1_info);
        let l1_versions = l1_known_version_tags(l1_info);

        self.compare_versions(
            &component_version,
            &l1_latest_version,
            &l1_latest_version,
            &l1_versions,
        )
    }

    fn assess_outdated_version(
        &self,
        l1_info: &L1VersionInfo,
        mainline_version: Option<String>,
    ) -> OutdatedVersionAssessment {
        let current_version = resolved_component_version(l1_info);
        let latest_version = Some(resolved_l1_latest_version(l1_info));
        let major_version_gap = latest_version.as_deref().and_then(|latest| {
            let current = VersionParser::parse(&current_version).ok()?;
            let latest = VersionParser::parse(latest).ok()?;
            if latest.major >= current.major {
                Some(latest.major - current.major)
            } else {
                Some(0)
            }
        });
        let is_outdated = major_version_gap
            .map(|gap| gap >= OUTDATED_MAJOR_VERSION_THRESHOLD)
            .unwrap_or(false);

        OutdatedVersionAssessment {
            current_version,
            latest_version,
            latest_version_source: Some("l1_repo".to_string()),
            mainline_version_source: mainline_version.as_ref().map(|_| "l0_repo".to_string()),
            mainline_version,
            major_version_gap,
            threshold_major_versions: OUTDATED_MAJOR_VERSION_THRESHOLD,
            is_outdated,
        }
    }

    fn assess_lts(&self, l1_info: &L1VersionInfo) -> LtsAssessment {
        LtsAssessment {
            is_lts: l1_info.is_lts,
            source: "l1_repo".to_string(),
            evidence: l1_info.lts_evidence.clone(),
        }
    }

    fn generate_partial_recommendations(
        &self,
        _maintenance_status: &MaintenanceStatus,
        outdated_version: &OutdatedVersionAssessment,
        lts: &LtsAssessment,
    ) -> Vec<String> {
        let mut recommendations =
            vec!["缺少 L0 版本/生命周期证据，停维判断和主线版本信息不完整".to_string()];

        if outdated_version.is_outdated {
            recommendations.push(format!(
                "当前组件版本与 L1 最新版本相差 {} 个大版本，已达到过时版本阈值 {}，建议规划升级",
                outdated_version.major_version_gap.unwrap_or(0),
                outdated_version.threshold_major_versions
            ));
        } else if outdated_version.latest_version.is_none() {
            recommendations.push("未能从 L1 仓库历史版本确认过时版本状态".to_string());
        }

        match lts.is_lts {
            Some(true) => recommendations.push("当前版本识别为 LTS/长期维护版本".to_string()),
            Some(false) => recommendations.push("当前版本未识别为 LTS/长期维护版本".to_string()),
            None => recommendations.push("未能从 L1 仓库信息确认当前版本是否为 LTS".to_string()),
        }

        recommendations
    }

    fn assess_maintenance_status(
        &self,
        current_version: &str,
        notices: &[MaintenanceNotice],
    ) -> MaintenanceStatus {
        let current_series = version_to_series(current_version);
        let matched_notice = notices
            .iter()
            .find(|notice| {
                notice
                    .series
                    .as_deref()
                    .map(|series| series == current_series)
                    .unwrap_or(false)
                    || notice
                        .version
                        .as_deref()
                        .map(|version| version == current_version)
                        .unwrap_or(false)
            })
            .cloned()
            .or_else(|| {
                notices
                    .iter()
                    .find(|notice| notice.series.is_none())
                    .cloned()
            });

        let (status, confidence) = if let Some(notice) = matched_notice.as_ref() {
            let status = classify_notice_status(notice.support_until.as_deref());
            let confidence = if notice.support_until.is_some()
                && (notice.series.is_some() || notice.version.is_some())
            {
                "HIGH"
            } else {
                "MEDIUM"
            };
            (status, confidence.to_string())
        } else if notices.is_empty() {
            ("UNKNOWN".to_string(), "LOW".to_string())
        } else {
            ("UNKNOWN".to_string(), "MEDIUM".to_string())
        };

        MaintenanceStatus {
            status,
            stop_maintenance_detected: matched_notice.is_some(),
            matched_notice,
            evidence: notices.iter().take(10).cloned().collect(),
            confidence,
        }
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
                        if entry_desc_lower.contains(&format!("cve-{}", year))
                            || entry_desc_lower.contains(&format!("cve {}", year))
                        {
                            // 进一步检查编号是否接近（可能是批量修复）
                            if let Some(mentioned_cves) =
                                Self::extract_cve_numbers(&entry_desc_lower)
                            {
                                for mentioned_number in mentioned_cves {
                                    // 如果编号差距在 100 以内，可能是相关修复
                                    if let Ok(num) = number.parse::<i32>() {
                                        if let Ok(mentioned_num) = mentioned_number.parse::<i32>() {
                                            if (num - mentioned_num).abs() <= 100 {
                                                // 这是一个启发式判断，可能需要人工确认
                                                // 但为了保守起见，我们不在这里返回 true
                                                // 只有精确匹配才返回 true
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 默认：未在上游修复
        Ok(false)
    }

    /// 解析 CVE 编号
    ///
    /// 从 CVE-YYYY-NNNNN 格式中提取年份和编号
    fn parse_cve_id(cve_id: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = cve_id.split('-').collect();
        if parts.len() >= 3 && parts[0].to_lowercase() == "cve" {
            Some((parts[1].to_string(), parts[2].to_string()))
        } else {
            None
        }
    }

    /// 从文本中提取 CVE 编号
    fn extract_cve_numbers(text: &str) -> Option<Vec<String>> {
        let re = regex::Regex::new(r"cve-\d{4}-(\d{4,})").ok()?;
        let numbers: Vec<String> = re
            .captures_iter(text)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect();

        if numbers.is_empty() {
            None
        } else {
            Some(numbers)
        }
    }

    /// 生成升级建议
    fn generate_recommendations(
        &self,
        version_comparison: &VersionComparison,
        patch_analysis: &PatchAnalysis,
        cve_analysis: &CveAnalysis,
        upgradable_versions: &[UpgradableVersion],
        maintenance_status: &MaintenanceStatus,
        outdated_version: &OutdatedVersionAssessment,
        lts: &LtsAssessment,
    ) -> Result<Vec<String>> {
        let mut recommendations = Vec::new();

        if let Some(notice) = maintenance_status.matched_notice.as_ref() {
            let until = notice
                .support_until
                .as_deref()
                .map(|value| format!("，维护截止日期为 {}", value))
                .unwrap_or_default();
            recommendations.push(format!(
                "识别到当前组件版本相关停维信息{}，来源：{}",
                until, notice.source
            ));
        }

        if outdated_version.is_outdated {
            recommendations.push(format!(
                "当前组件版本与 L1 最新版本相差 {} 个大版本，已达到过时版本阈值 {}，建议规划升级",
                outdated_version.major_version_gap.unwrap_or(0),
                outdated_version.threshold_major_versions
            ));
        }

        match lts.is_lts {
            Some(true) => recommendations.push("当前版本识别为 LTS/长期维护版本".to_string()),
            Some(false) => recommendations.push("当前版本未识别为 LTS/长期维护版本".to_string()),
            None => recommendations.push("未能从 L1 仓库信息确认当前版本是否为 LTS".to_string()),
        }

        // 1. 版本落后建议
        if version_comparison.is_outdated {
            if version_comparison.behind_count == 0 {
                recommendations.push("当前组件版本已与 L1 最新版本对齐".to_string());
            } else if version_comparison.behind_count == 1 {
                recommendations.push("当前组件版本落后 L1 1 个版本，建议规划同步".to_string());
            } else {
                recommendations.push(format!(
                    "当前组件版本落后 L1 {} 个版本，建议尽快规划同步",
                    version_comparison.behind_count
                ));
            }
        }

        // 2. 安全更新建议
        let security_releases: Vec<_> = upgradable_versions
            .iter()
            .filter(|v| v.is_security_release)
            .collect();

        if !security_releases.is_empty() {
            recommendations.push(format!(
                "发现 {} 个安全更新版本，建议优先升级",
                security_releases.len()
            ));
        }

        // 3. CVE 修复建议
        if !cve_analysis.fixed_in_upstream.is_empty() {
            recommendations.push(format!(
                "{} 个 CVE 已在上游修复，升级后可移除相关补丁",
                cve_analysis.fixed_in_upstream.len()
            ));
        }

        if !cve_analysis.not_fixed_in_upstream.is_empty() {
            recommendations.push(format!(
                "{} 个 CVE 尚未在上游修复，升级后仍需保留相关补丁",
                cve_analysis.not_fixed_in_upstream.len()
            ));
        }

        // 4. 补丁清理建议
        if patch_analysis.can_be_removed_after_upgrade > 0 {
            recommendations.push(format!(
                "升级后可移除 {} 个已合并到上游的补丁",
                patch_analysis.can_be_removed_after_upgrade
            ));
        }

        // 5. Breaking changes 警告
        let has_breaking_changes = upgradable_versions
            .iter()
            .any(|v| !v.breaking_changes.is_empty());

        if has_breaking_changes {
            recommendations
                .push("注意：部分可升级版本包含不兼容的变更，升级前请仔细评估影响".to_string());
        }

        // 6. 如果没有任何建议，添加默认建议
        if recommendations.is_empty() {
            recommendations.push("当前版本状态良好，暂无升级建议".to_string());
        }

        Ok(recommendations)
    }
}

fn resolved_component_version(l1_info: &L1VersionInfo) -> String {
    l1_info
        .component_version
        .as_ref()
        .filter(|version| !version.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| l1_info.current_version.clone())
}

fn resolved_l1_latest_version(l1_info: &L1VersionInfo) -> String {
    l1_info
        .latest_version
        .as_ref()
        .filter(|version| !version.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| l1_info.current_version.clone())
}

fn l1_known_version_tags(l1_info: &L1VersionInfo) -> Vec<VersionTag> {
    let mut versions = Vec::new();

    push_unique_version(&mut versions, &l1_info.current_version);
    if let Some(latest) = l1_info.latest_version.as_ref() {
        push_unique_version(&mut versions, latest);
    }
    for version in &l1_info.known_versions {
        push_unique_version(&mut versions, version);
    }

    versions
        .into_iter()
        .filter_map(|version| {
            let parsed = VersionParser::parse(&version).ok()?;
            Some(VersionTag {
                version,
                date: Utc::now(),
                changelog: "l1 repository version history".to_string(),
                is_stable: parsed.is_stable(),
            })
        })
        .collect()
}

fn push_unique_version(versions: &mut Vec<String>, version: &str) {
    let version = version.trim();
    if version.is_empty() || versions.iter().any(|item| item == version) {
        return;
    }
    versions.push(version.to_string());
}

pub fn extract_versions_from_text(text: &str) -> Vec<String> {
    let re = Regex::new(
        r"(?ix)
        (?:^|[^\d])
        (?:v|version|release|tag|upgrade(?:d)?(?:\s+to)?|update(?:d)?(?:\s+to)?|版本|升级到|更新到)?
        \s*
        v?
        (?P<version>\d+\.\d+(?:\.\d+)?(?:[-_\.]?(?:alpha|beta|rc)\d*)?)
        ",
    )
    .expect("version extraction regex");

    let mut versions = Vec::new();
    for cap in re.captures_iter(text) {
        let Some(matched) = cap.name("version") else {
            continue;
        };
        let value = matched
            .as_str()
            .trim_matches(['.', ',', ';', ')', ']', '}']);
        if is_probable_date(value) {
            continue;
        }
        if VersionParser::parse(value).is_ok() && !versions.iter().any(|v| v == value) {
            versions.push(value.to_string());
        }
    }
    versions
}

pub fn extract_maintenance_notices(
    text: &str,
    source: impl Into<String>,
) -> Vec<MaintenanceNotice> {
    let source = source.into();
    text.lines()
        .flat_map(split_into_notice_fragments)
        .filter_map(|fragment| build_maintenance_notice(fragment, &source))
        .collect()
}

pub fn version_to_series(version: &str) -> String {
    let versions = extract_versions_from_text(version);
    let normalized = versions.first().map(String::as_str).unwrap_or(version);
    let normalized = normalized
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V');
    let parts: Vec<&str> = normalized.split('.').collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[0], parts[1])
    } else {
        normalized.to_string()
    }
}

fn split_into_notice_fragments(line: &str) -> Vec<&str> {
    line.split(['。', ';', '；'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

fn build_maintenance_notice(fragment: &str, source: &str) -> Option<MaintenanceNotice> {
    if !contains_maintenance_stop_signal(fragment) {
        return None;
    }
    let version = extract_versions_from_text(fragment).into_iter().next();
    let series = version.as_deref().map(version_to_series);
    let support_until = extract_support_until(fragment);
    let status = classify_notice_status(support_until.as_deref());

    Some(MaintenanceNotice {
        version,
        series,
        support_until,
        status,
        source: source.to_string(),
        evidence: fragment.chars().take(240).collect(),
    })
}

fn contains_maintenance_stop_signal(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("end-of-life")
        || lower.contains("end of life")
        || lower.contains("eol")
        || lower.contains("end of support")
        || lower.contains("out of support")
        || lower.contains("no longer supported")
        || lower.contains("unsupported")
        || text.contains("不再维护")
        || text.contains("停止维护")
        || text.contains("停止支持")
        || text.contains("停维")
        || text.contains("维护截止")
        || text.contains("支持截止")
        || text.contains("生命周期结束")
        || text.contains("结束维护")
        || text.contains("终止维护")
}

fn extract_support_until(text: &str) -> Option<String> {
    let ymd = Regex::new(
        r"(?x)
        (?P<year>\d{4})
        [年\-/\.]
        (?P<month>\d{1,2})
        [月\-/\.]
        (?P<day>\d{1,2})
        日?
        ",
    )
    .expect("ymd regex");
    if let Some(cap) = ymd.captures(text) {
        let year = cap.name("year")?.as_str().parse::<i32>().ok()?;
        let month = cap.name("month")?.as_str().parse::<u32>().ok()?;
        let day = cap.name("day")?.as_str().parse::<u32>().ok()?;
        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
            return Some(date.format("%Y-%m-%d").to_string());
        }
    }

    let day_month_year =
        Regex::new(r"(?i)\b(?P<day>\d{1,2})\s+(?P<month>[a-z]{3,9})\s+(?P<year>\d{4})\b")
            .expect("day month year regex");
    if let Some(cap) = day_month_year.captures(text) {
        let raw = format!(
            "{} {} {}",
            cap.name("day")?.as_str(),
            cap.name("month")?.as_str(),
            cap.name("year")?.as_str()
        );
        for fmt in ["%d %b %Y", "%d %B %Y"] {
            if let Ok(date) = NaiveDate::parse_from_str(&raw, fmt) {
                return Some(date.format("%Y-%m-%d").to_string());
            }
        }
    }

    None
}

fn classify_notice_status(support_until: Option<&str>) -> String {
    let Some(support_until) = support_until else {
        return "END_OF_MAINTENANCE_NOTICE".to_string();
    };
    let Some(date) = parse_support_until_date(support_until) else {
        return "END_OF_MAINTENANCE_NOTICE".to_string();
    };
    if Utc::now().date_naive() <= date {
        "SCHEDULED_EOL".to_string()
    } else {
        "OUT_OF_SUPPORT".to_string()
    }
}

fn parse_support_until_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn is_probable_date(value: &str) -> bool {
    VersionParser::parse(value)
        .map(|version| version.major >= 1000)
        .unwrap_or(false)
}

impl Default for L1VsL0Comparator {
    fn default() -> Self {
        Self::new()
    }
}

/// 版本对比结果（内部使用）
#[derive(Debug)]
struct VersionComparison {
    behind_count: u32,
    is_outdated: bool,
    #[allow(dead_code)]
    has_newer_stable: bool,
    #[allow(dead_code)]
    has_newer_latest: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_l1_vs_l0_comparator_creation() {
        let _comparator = L1VsL0Comparator::new();
        // 基本创建测试
    }

    #[tokio::test]
    async fn test_version_comparison() {
        let comparator = L1VsL0Comparator::new();

        // 准备测试数据
        let l0_info = L0VersionInfo {
            package_name: "nginx".to_string(),
            latest_stable: "1.24.0".to_string(),
            latest_version: "1.25.0-beta".to_string(),
            all_versions: vec![
                VersionTag {
                    version: "1.22.0".to_string(),
                    date: Utc::now(),
                    changelog: "Bug fixes".to_string(),
                    is_stable: true,
                },
                VersionTag {
                    version: "1.23.0".to_string(),
                    date: Utc::now(),
                    changelog: "New features".to_string(),
                    is_stable: true,
                },
                VersionTag {
                    version: "1.24.0".to_string(),
                    date: Utc::now(),
                    changelog: "Security fixes".to_string(),
                    is_stable: true,
                },
            ],
            changelogs: HashMap::new(),
            maintenance_notices: vec![],
        };

        let l1_info = L1VersionInfo {
            package_name: "nginx".to_string(),
            current_version: "1.22.0".to_string(),
            component_version: None,
            latest_version: Some("1.24.0".to_string()),
            known_versions: vec!["1.22.0".to_string(), "1.24.0".to_string()],
            is_lts: Some(false),
            lts_evidence: vec!["L1 分支未包含 LTS 标识".to_string()],
            patches: vec![],
            cve_patches: vec![],
        };

        // 执行对比
        let report = comparator.compare(&l0_info, &l1_info).await.unwrap();

        // 验证结果
        assert_eq!(report.package_name, "nginx");
        assert_eq!(report.current_version, "1.22.0");
        assert_eq!(report.latest_stable, "1.24.0");
        assert_eq!(report.latest_version, "1.24.0");
        assert_eq!(report.mainline_version.as_deref(), Some("1.25.0-beta"));
        assert!(report.version_behind > 0);
        assert!(!report.recommendations.is_empty());
    }

    #[test]
    fn test_compare_without_l0_keeps_l1_lifecycle_assessments() {
        let comparator = L1VsL0Comparator::new();
        let l1_info = L1VersionInfo {
            package_name: "binutils".to_string(),
            current_version: "2.34".to_string(),
            component_version: None,
            latest_version: Some("5.34".to_string()),
            known_versions: vec!["2.34".to_string(), "5.34".to_string()],
            is_lts: Some(true),
            lts_evidence: vec!["L1 分支包含 LTS 标识: openEuler-20.03-LTS-SP4".to_string()],
            patches: vec![],
            cve_patches: vec![],
        };

        let report = comparator.compare_without_l0(&l1_info);

        assert_eq!(report.package_name, "binutils");
        assert_eq!(report.maintenance_status.status, "UNKNOWN");
        assert!(!report.maintenance_status.stop_maintenance_detected);
        assert_eq!(report.lts.is_lts, Some(true));
        assert!(report.outdated_version.is_outdated);
        assert_eq!(report.outdated_version.major_version_gap, Some(3));
        assert!(report
            .recommendations
            .iter()
            .any(|item| item.contains("缺少 L0 版本/生命周期证据")));
    }

    #[test]
    fn test_outdated_assessment_uses_l2_l1_and_keeps_l0_reference() {
        let comparator = L1VsL0Comparator::new();
        let l1_info = L1VersionInfo {
            package_name: "demo".to_string(),
            current_version: "4.2.0".to_string(),
            component_version: Some("1.2.0".to_string()),
            latest_version: Some("4.2.0".to_string()),
            known_versions: vec!["1.2.0".to_string(), "4.2.0".to_string()],
            is_lts: None,
            lts_evidence: vec![],
            patches: vec![],
            cve_patches: vec![],
        };

        let report = comparator.compare_without_l0(&l1_info);

        assert_eq!(report.current_version, "1.2.0");
        assert_eq!(report.latest_version, "4.2.0");
        assert_eq!(report.mainline_version, None);
        assert!(report.outdated_version.is_outdated);
        assert_eq!(report.outdated_version.major_version_gap, Some(3));
    }

    #[tokio::test]
    async fn test_find_upgradable_versions() {
        let comparator = L1VsL0Comparator::new();

        let all_versions = vec![
            VersionTag {
                version: "1.20.0".to_string(),
                date: Utc::now(),
                changelog: "Old version".to_string(),
                is_stable: true,
            },
            VersionTag {
                version: "1.22.0".to_string(),
                date: Utc::now(),
                changelog: "Bug fixes".to_string(),
                is_stable: true,
            },
            VersionTag {
                version: "1.23.0".to_string(),
                date: Utc::now(),
                changelog: "Security update with CVE fixes".to_string(),
                is_stable: true,
            },
            VersionTag {
                version: "1.24.0-beta".to_string(),
                date: Utc::now(),
                changelog: "Beta release".to_string(),
                is_stable: false,
            },
        ];

        let upgradable = comparator
            .find_upgradable_versions("1.20.0", &all_versions)
            .unwrap();

        // 应该只包含稳定版本
        assert_eq!(upgradable.len(), 2); // 1.22.0 和 1.23.0
        assert!(upgradable.iter().any(|v| v.version == "1.23.0"));
        assert!(
            upgradable
                .iter()
                .find(|v| v.version == "1.23.0")
                .unwrap()
                .is_security_release
        );
    }

    #[tokio::test]
    async fn test_patch_analysis() {
        let comparator = L1VsL0Comparator::new();

        // 准备测试数据
        let patches = vec![
            PatchInfo {
                filename: "0001-backport-fix-buffer-overflow.patch".to_string(),
                description: "Backport of upstream commit abc123def456: Fix buffer overflow"
                    .to_string(),
                applied: true,
                content_hash: Some("hash1".to_string()),
            },
            PatchInfo {
                filename: "0002-CVE-2023-1234.patch".to_string(),
                description: "Fix CVE-2023-1234 vulnerability".to_string(),
                applied: true,
                content_hash: Some("hash2".to_string()),
            },
            PatchInfo {
                filename: "0003-custom-feature.patch".to_string(),
                description: "Add custom feature for enterprise".to_string(),
                applied: true,
                content_hash: Some("hash3".to_string()),
            },
        ];

        let mut changelogs = HashMap::new();
        changelogs.insert(
            "1.23.0".to_string(),
            vec![
                ChangelogEntry {
                    entry_type: "bugfix".to_string(),
                    description: "Fix buffer overflow in parser".to_string(),
                    commit_sha: Some("abc123def456".to_string()),
                },
                ChangelogEntry {
                    entry_type: "security".to_string(),
                    description: "Fix CVE-2023-1234: Memory corruption".to_string(),
                    commit_sha: Some("def456abc123".to_string()),
                },
            ],
        );

        let upgradable_versions = vec![UpgradableVersion {
            version: "1.23.0".to_string(),
            release_date: Utc::now(),
            is_security_release: true,
            breaking_changes: Vec::new(),
        }];

        // 执行补丁分析
        let analysis = comparator
            .analyze_patches(&patches, &changelogs, &upgradable_versions)
            .unwrap();

        // 验证结果
        assert_eq!(analysis.total_patches, 3);

        // 应该识别出 2 个已合并的补丁（backport 和 CVE）
        assert!(!analysis.merged_in_upstream.is_empty());

        // 应该识别出至少 1 个仍需保留的补丁（custom feature）
        assert!(!analysis.still_needed.is_empty());

        // 可移除的补丁数应该等于已合并的补丁数
        assert_eq!(
            analysis.can_be_removed_after_upgrade,
            analysis.merged_in_upstream.len()
        );
    }

    #[tokio::test]
    async fn test_full_comparison_with_patches() {
        let comparator = L1VsL0Comparator::new();

        // 准备完整的测试数据
        let l0_info = L0VersionInfo {
            package_name: "nginx".to_string(),
            latest_stable: "1.24.0".to_string(),
            latest_version: "1.25.0-beta".to_string(),
            all_versions: vec![
                VersionTag {
                    version: "1.22.0".to_string(),
                    date: Utc::now(),
                    changelog: "Bug fixes".to_string(),
                    is_stable: true,
                },
                VersionTag {
                    version: "1.23.0".to_string(),
                    date: Utc::now(),
                    changelog: "Security fixes including CVE-2023-1234".to_string(),
                    is_stable: true,
                },
                VersionTag {
                    version: "1.24.0".to_string(),
                    date: Utc::now(),
                    changelog: "Performance improvements".to_string(),
                    is_stable: true,
                },
            ],
            changelogs: {
                let mut map = HashMap::new();
                map.insert(
                    "1.23.0".to_string(),
                    vec![ChangelogEntry {
                        entry_type: "security".to_string(),
                        description: "Fix CVE-2023-1234".to_string(),
                        commit_sha: Some("abc123".to_string()),
                    }],
                );
                map
            },
            maintenance_notices: vec![],
        };

        let l1_info = L1VersionInfo {
            package_name: "nginx".to_string(),
            current_version: "1.22.0".to_string(),
            component_version: None,
            latest_version: Some("1.24.0".to_string()),
            known_versions: vec!["1.22.0".to_string(), "1.24.0".to_string()],
            is_lts: Some(true),
            lts_evidence: vec!["L1 分支包含 LTS 标识".to_string()],
            patches: vec![PatchInfo {
                filename: "CVE-2023-1234.patch".to_string(),
                description: "Fix CVE-2023-1234 vulnerability".to_string(),
                applied: true,
                content_hash: Some("hash1".to_string()),
            }],
            cve_patches: vec![CveInfo {
                cve_id: "CVE-2023-1234".to_string(),
                patch_file: "CVE-2023-1234.patch".to_string(),
                description: "Memory corruption vulnerability".to_string(),
                severity: Some("High".to_string()),
            }],
        };

        // 执行完整对比
        let report = comparator.compare(&l0_info, &l1_info).await.unwrap();

        // 验证报告
        assert_eq!(report.package_name, "nginx");
        assert_eq!(report.current_version, "1.22.0");
        assert_eq!(report.latest_stable, "1.24.0");
        assert!(report.version_behind > 0);

        // 验证补丁分析
        assert_eq!(report.patch_analysis.total_patches, 1);

        // 验证 CVE 分析
        assert_eq!(report.cve_analysis.total_cves, 1);
        // CVE-2023-1234 应该被识别为已在上游修复（在 1.23.0 的 changelog 中）
        assert_eq!(report.cve_analysis.fixed_in_upstream.len(), 1);
        assert_eq!(report.cve_analysis.not_fixed_in_upstream.len(), 0);

        // 验证建议
        assert!(!report.recommendations.is_empty());
        // 应该包含 CVE 修复建议
        assert!(report
            .recommendations
            .iter()
            .any(|r| r.contains("CVE") && r.contains("上游修复")));
    }

    #[tokio::test]
    async fn test_lifecycle_outdated_and_lts_assessment() {
        let comparator = L1VsL0Comparator::new();
        let notices = extract_maintenance_notices(
            "Version 1.0 will be no longer supported after 2030-01-01",
            "official_page:https://example.com/lifecycle",
        );

        let l0_info = L0VersionInfo {
            package_name: "demo".to_string(),
            latest_stable: "5.0.0".to_string(),
            latest_version: "5.0.0".to_string(),
            all_versions: vec![VersionTag {
                version: "5.0.0".to_string(),
                date: Utc::now(),
                changelog: "release".to_string(),
                is_stable: true,
            }],
            changelogs: HashMap::new(),
            maintenance_notices: notices,
        };

        let l1_info = L1VersionInfo {
            package_name: "demo".to_string(),
            current_version: "1.0.0".to_string(),
            component_version: None,
            latest_version: Some("4.0.0".to_string()),
            known_versions: vec!["1.0.0".to_string(), "4.0.0".to_string()],
            is_lts: Some(true),
            lts_evidence: vec!["L1 分支包含 LTS: openEuler-22.03-LTS".to_string()],
            patches: vec![],
            cve_patches: vec![],
        };

        let report = comparator.compare(&l0_info, &l1_info).await.unwrap();
        assert!(report.maintenance_status.stop_maintenance_detected);
        assert_eq!(report.maintenance_status.status, "SCHEDULED_EOL");
        assert!(report.outdated_version.is_outdated);
        assert_eq!(report.outdated_version.major_version_gap, Some(3));
        assert_eq!(report.lts.is_lts, Some(true));
    }

    #[tokio::test]
    async fn test_cve_analysis_fixed_in_upstream() {
        let comparator = L1VsL0Comparator::new();

        // 准备测试数据：CVE 已在上游修复
        let cve_patches = vec![
            CveInfo {
                cve_id: "CVE-2023-1234".to_string(),
                patch_file: "CVE-2023-1234.patch".to_string(),
                description: "Buffer overflow".to_string(),
                severity: Some("High".to_string()),
            },
            CveInfo {
                cve_id: "CVE-2023-5678".to_string(),
                patch_file: "CVE-2023-5678.patch".to_string(),
                description: "Use after free".to_string(),
                severity: Some("Critical".to_string()),
            },
        ];

        let mut changelogs = HashMap::new();
        changelogs.insert(
            "1.23.0".to_string(),
            vec![
                ChangelogEntry {
                    entry_type: "security".to_string(),
                    description: "Fix CVE-2023-1234: Buffer overflow in parser".to_string(),
                    commit_sha: Some("abc123".to_string()),
                },
                ChangelogEntry {
                    entry_type: "security".to_string(),
                    description: "Fix CVE-2023-5678: Memory corruption".to_string(),
                    commit_sha: Some("def456".to_string()),
                },
            ],
        );

        // 执行 CVE 分析
        let analysis = comparator
            .analyze_cve_patches(&cve_patches, &changelogs)
            .unwrap();

        // 验证结果
        assert_eq!(analysis.total_cves, 2);
        assert_eq!(analysis.fixed_in_upstream.len(), 2);
        assert_eq!(analysis.not_fixed_in_upstream.len(), 0);

        // 验证具体的 CVE
        assert!(analysis
            .fixed_in_upstream
            .iter()
            .any(|c| c.cve_id == "CVE-2023-1234"));
        assert!(analysis
            .fixed_in_upstream
            .iter()
            .any(|c| c.cve_id == "CVE-2023-5678"));
    }

    #[tokio::test]
    async fn test_cve_analysis_not_fixed_in_upstream() {
        let comparator = L1VsL0Comparator::new();

        // 准备测试数据：CVE 未在上游修复
        let cve_patches = vec![CveInfo {
            cve_id: "CVE-2023-9999".to_string(),
            patch_file: "CVE-2023-9999.patch".to_string(),
            description: "Custom vulnerability".to_string(),
            severity: Some("Medium".to_string()),
        }];

        let mut changelogs = HashMap::new();
        changelogs.insert(
            "1.23.0".to_string(),
            vec![ChangelogEntry {
                entry_type: "feature".to_string(),
                description: "Add new feature".to_string(),
                commit_sha: Some("abc123".to_string()),
            }],
        );

        // 执行 CVE 分析
        let analysis = comparator
            .analyze_cve_patches(&cve_patches, &changelogs)
            .unwrap();

        // 验证结果
        assert_eq!(analysis.total_cves, 1);
        assert_eq!(analysis.fixed_in_upstream.len(), 0);
        assert_eq!(analysis.not_fixed_in_upstream.len(), 1);
        assert_eq!(analysis.not_fixed_in_upstream[0].cve_id, "CVE-2023-9999");
    }

    #[tokio::test]
    async fn test_cve_analysis_mixed_status() {
        let comparator = L1VsL0Comparator::new();

        // 准备测试数据：部分 CVE 已修复，部分未修复
        let cve_patches = vec![
            CveInfo {
                cve_id: "CVE-2023-1234".to_string(),
                patch_file: "CVE-2023-1234.patch".to_string(),
                description: "Fixed in upstream".to_string(),
                severity: Some("High".to_string()),
            },
            CveInfo {
                cve_id: "CVE-2023-9999".to_string(),
                patch_file: "CVE-2023-9999.patch".to_string(),
                description: "Not fixed in upstream".to_string(),
                severity: Some("Medium".to_string()),
            },
        ];

        let mut changelogs = HashMap::new();
        changelogs.insert(
            "1.23.0".to_string(),
            vec![ChangelogEntry {
                entry_type: "security".to_string(),
                description: "Fix CVE-2023-1234".to_string(),
                commit_sha: Some("abc123".to_string()),
            }],
        );

        // 执行 CVE 分析
        let analysis = comparator
            .analyze_cve_patches(&cve_patches, &changelogs)
            .unwrap();

        // 验证结果
        assert_eq!(analysis.total_cves, 2);
        assert_eq!(analysis.fixed_in_upstream.len(), 1);
        assert_eq!(analysis.not_fixed_in_upstream.len(), 1);
        assert_eq!(analysis.fixed_in_upstream[0].cve_id, "CVE-2023-1234");
        assert_eq!(analysis.not_fixed_in_upstream[0].cve_id, "CVE-2023-9999");
    }

    #[test]
    fn test_parse_cve_id() {
        assert_eq!(
            L1VsL0Comparator::parse_cve_id("CVE-2023-1234"),
            Some(("2023".to_string(), "1234".to_string()))
        );
        assert_eq!(
            L1VsL0Comparator::parse_cve_id("CVE-2024-56789"),
            Some(("2024".to_string(), "56789".to_string()))
        );
        assert_eq!(L1VsL0Comparator::parse_cve_id("invalid"), None);
    }

    #[test]
    fn test_extract_cve_numbers() {
        // 注意：extract_cve_numbers 期望小写文本
        let text = "fix cve-2023-1234 and cve-2023-5678";
        let numbers = L1VsL0Comparator::extract_cve_numbers(text);
        assert!(numbers.is_some());
        let numbers = numbers.unwrap();
        assert_eq!(numbers.len(), 2);
        assert!(numbers.contains(&"1234".to_string()));
        assert!(numbers.contains(&"5678".to_string()));

        let text = "no cve here";
        let numbers = L1VsL0Comparator::extract_cve_numbers(text);
        assert!(numbers.is_none());
    }
}
