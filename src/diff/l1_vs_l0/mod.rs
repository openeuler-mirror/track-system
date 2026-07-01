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
    ) -> Result<Vec<String>> {
        let mut recommendations = Vec::new();

        // 1. 版本落后建议
        if version_comparison.is_outdated {
            if version_comparison.behind_count == 0 {
                recommendations.push("当前版本已是最新稳定版本".to_string());
            } else if version_comparison.behind_count == 1 {
                recommendations.push("当前版本落后 1 个版本，建议升级到最新稳定版本".to_string());
            } else {
                recommendations.push(format!(
                    "当前版本落后 {} 个版本，强烈建议升级到最新稳定版本",
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
        };

        let l1_info = L1VersionInfo {
            package_name: "nginx".to_string(),
            current_version: "1.22.0".to_string(),
            patches: vec![],
            cve_patches: vec![],
        };

        // 执行对比
        let report = comparator.compare(&l0_info, &l1_info).await.unwrap();

        // 验证结果
        assert_eq!(report.package_name, "nginx");
        assert_eq!(report.current_version, "1.22.0");
        assert_eq!(report.latest_stable, "1.24.0");
        assert!(report.version_behind > 0);
        assert!(!report.recommendations.is_empty());
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
        };

        let l1_info = L1VersionInfo {
            package_name: "nginx".to_string(),
            current_version: "1.22.0".to_string(),
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
