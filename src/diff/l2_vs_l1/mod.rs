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

//! L2 vs L1 内容对比模块
//!
//! 用于对比企业发行版（L2）相对于社区发行版（L1）的内容差异

use crate::snapshot::types::{CommitEntry, FileEntry, RepositorySnapshot};
use crate::utils::spec::{SpecComparison, SpecParser};
use crate::utils::version::VersionParser;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// L1 快照（社区发行版）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1Snapshot {
    /// 软件包名称
    pub package_name: String,
    /// 版本号
    pub version: String,
    /// spec 文件内容
    pub spec_content: String,
    /// spec 文件哈希
    pub spec_sha256: String,
    /// patch 文件列表
    pub patches: Vec<PatchFile>,
    /// 源文件列表
    pub source_files: Vec<SourceFile>,
    /// commit 记录列表
    pub commits: Vec<CommitEntry>,
    /// 快照时间
    pub snapshot_at: DateTime<Utc>,
}

/// L2 快照（企业发行版）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Snapshot {
    /// 软件包名称
    pub package_name: String,
    /// 版本号
    pub version: String,
    /// spec 文件内容
    pub spec_content: String,
    /// spec 文件哈希
    pub spec_sha256: String,
    /// patch 文件列表
    pub patches: Vec<PatchFile>,
    /// 源文件列表
    pub source_files: Vec<SourceFile>,
    /// 定制内容
    pub customizations: Vec<Customization>,
    /// commit 记录列表
    pub commits: Vec<CommitEntry>,
    /// 快照时间
    pub snapshot_at: DateTime<Utc>,
}

/// Patch 文件
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatchFile {
    /// 文件名
    pub filename: String,
    /// 文件路径
    pub path: String,
    /// 内容哈希（SHA256）
    pub content_hash: String,
    /// 文件大小
    pub size: u64,
    /// 是否已应用
    pub applied: bool,
}

/// 源文件
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceFile {
    /// 文件名
    pub filename: String,
    /// 文件路径
    pub path: String,
    /// 内容哈希（SHA256）
    pub content_hash: String,
    /// 文件大小
    pub size: u64,
}

/// 定制内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customization {
    /// 定制类型
    pub customization_type: CustomizationType,
    /// 描述
    pub description: String,
    /// 影响的文件
    pub affected_files: Vec<String>,
}

/// 定制类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CustomizationType {
    /// 版本变更
    VersionChange,
    /// 功能修改
    FeatureModification,
    /// 配置修改
    ConfigurationChange,
    /// 安全加固
    SecurityHardening,
    /// 性能优化
    PerformanceOptimization,
    /// 其他
    Other,
}

/// Commit 差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitDiff {
    /// L1 中的总 commits 数
    pub l1_commits_count: usize,
    /// L2 中的总 commits 数
    pub l2_commits_count: usize,
    /// L2 落后于 L1 的 commits
    pub behind_commits: Vec<CommitEntry>,
    /// 作为基线的 commit（version-release 匹配的 commit）
    pub base_commit: Option<CommitEntry>,
    /// 基线 commit 的版本-发行信息
    pub base_version_release: Option<(String, Option<String>)>,
}

/// L2 vs L1 对比报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2VsL1Report {
    /// 报告 ID
    pub id: Option<i64>,
    /// 软件包名称
    pub package_name: String,
    /// spec 文件差异
    pub spec_diff: SpecDiff,
    /// patch 文件差异
    pub patch_diff: PatchDiff,
    /// 源文件差异
    pub source_diff: SourceDiff,
    /// 定制内容分析
    pub customization_analysis: CustomizationAnalysis,
    /// 同步建议
    pub sync_recommendations: Vec<SyncRecommendation>,
    /// 冲突列表
    pub conflicts: Vec<MergeConflict>,
    /// commit 差异
    pub commit_diff: CommitDiff,
    /// 生成时间
    pub created_at: DateTime<Utc>,
}

/// spec 文件差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecDiff {
    /// 版本差异
    pub version_diff: Option<VersionDiff>,
    /// 内容哈希是否相同
    pub content_identical: bool,
    /// 差异摘要
    pub diff_summary: String,
    /// 关键变更
    pub key_changes: Vec<String>,
    /// 详细的 spec 对比结果
    pub detailed_comparison: Option<SpecComparison>,
    /// 新增的 BuildRequires
    pub build_requires_added: Vec<String>,
    /// 删除的 BuildRequires
    pub build_requires_removed: Vec<String>,
    /// 新增的 configure 选项
    pub configure_options_added: Vec<String>,
    /// 删除的 configure 选项
    pub configure_options_removed: Vec<String>,
}

/// 版本差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionDiff {
    /// L1 版本
    pub l1_version: String,
    /// L2 版本
    pub l2_version: String,
    /// 版本关系
    pub relationship: VersionRelationship,
}

/// 版本关系
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VersionRelationship {
    /// L2 版本更新
    L2Newer,
    /// L2 版本更旧
    L2Older,
    /// 版本相同
    Same,
    /// 无法比较
    Incomparable,
}

/// patch 文件差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchDiff {
    /// 总 patch 数（L1）
    pub l1_total: usize,
    /// 总 patch 数（L2）
    pub l2_total: usize,
    /// L2 新增的 patch
    pub l2_added: Vec<PatchFile>,
    /// L2 删除的 patch（L1 有但 L2 没有）
    pub l2_removed: Vec<PatchFile>,
    /// L2 修改的 patch（文件名相同但内容不同）
    pub l2_modified: Vec<PatchModification>,
    /// 相同的 patch
    pub identical: Vec<PatchFile>,
}

/// patch 修改
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchModification {
    /// 文件名
    pub filename: String,
    /// L1 的哈希
    pub l1_hash: String,
    /// L2 的哈希
    pub l2_hash: String,
}

/// 源文件差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDiff {
    /// 总源文件数（L1）
    pub l1_total: usize,
    /// 总源文件数（L2）
    pub l2_total: usize,
    /// L2 新增的源文件
    pub l2_added: Vec<SourceFile>,
    /// L2 删除的源文件
    pub l2_removed: Vec<SourceFile>,
    /// L2 修改的源文件
    pub l2_modified: Vec<SourceModification>,
}

/// 源文件修改
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceModification {
    /// 文件名
    pub filename: String,
    /// L1 的哈希
    pub l1_hash: String,
    /// L2 的哈希
    pub l2_hash: String,
}

/// 定制内容分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomizationAnalysis {
    /// 总定制数
    pub total_customizations: usize,
    /// 按类型分组的定制
    pub by_type: HashMap<String, Vec<Customization>>,
    /// 定制摘要
    pub summary: String,
}

/// 同步建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRecommendation {
    /// 优先级
    pub priority: SyncPriority,
    /// 建议类型
    pub recommendation_type: SyncType,
    /// 描述
    pub description: String,
    /// 影响的文件
    pub affected_files: Vec<String>,
    /// 预计工作量
    pub estimated_effort: EffortLevel,
}

/// 同步优先级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum SyncPriority {
    /// 紧急（安全问题）
    Critical,
    /// 高（重要功能）
    High,
    /// 中（一般更新）
    Medium,
    /// 低（可选更新）
    Low,
}

/// 同步类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyncType {
    /// 版本升级
    VersionUpgrade,
    /// 安全补丁
    SecurityPatch,
    /// Bug 修复
    BugFix,
    /// 新功能
    NewFeature,
    /// 配置更新
    ConfigUpdate,
}

/// 工作量级别
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EffortLevel {
    /// 低（< 1 小时）
    Low,
    /// 中（1-4 小时）
    Medium,
    /// 高（> 4 小时）
    High,
}

/// 合并冲突
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConflict {
    /// 冲突类型
    pub conflict_type: ConflictType,
    /// 描述
    pub description: String,
    /// 涉及的文件
    pub files: Vec<String>,
    /// 解决建议
    pub resolution_hint: String,
}

/// 冲突类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConflictType {
    /// 版本冲突
    VersionConflict,
    /// Patch 冲突
    PatchConflict,
    /// 文件修改冲突
    FileModificationConflict,
    /// 配置冲突
    ConfigurationConflict,
}

/// L2 vs L1 对比器
pub struct L2VsL1Comparator;

impl L2VsL1Comparator {
    /// 创建新的对比器
    pub fn new() -> Self {
        Self
    }

    /// 从 RepositorySnapshot 创建 L1 快照
    pub fn create_l1_snapshot(
        package_name: String,
        snapshot: &RepositorySnapshot,
    ) -> Result<L1Snapshot> {
        // 提取 spec 信息并在失败时记录错误日志
        let spec = match snapshot.spec.as_ref() {
            Some(s) => s,
            None => {
                tracing::error!(
                    tracking_id = snapshot.tracking_id,
                    origin = ?snapshot.origin,
                    files_count = snapshot.files.len(),
                    "创建 L1 快照失败：缺少 spec 文件"
                );
                return Err(anyhow!("L1 快照缺少 spec 文件"));
            }
        };

        // 提取版本号并在失败时记录错误日志
        let version = match spec.version.clone() {
            Some(v) => v,
            None => {
                tracing::error!(
                    tracking_id = snapshot.tracking_id,
                    spec_path = %spec.path,
                    spec_sha256 = %spec.sha256,
                    "创建 L1 快照失败：无法从 spec 文件提取版本号"
                );
                return Err(anyhow!("无法从 spec 文件提取版本号"));
            }
        };

        // 解码 spec 内容并在失败时记录错误日志
        use base64::Engine;
        let decoded = match base64::engine::general_purpose::STANDARD.decode(&spec.content_base64) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::error!(
                    tracking_id = snapshot.tracking_id,
                    spec_path = %spec.path,
                    spec_sha256 = %spec.sha256,
                    base64_len = spec.content_base64.len(),
                    error = %e,
                    "创建 L1 快照失败：解码 spec 内容失败"
                );
                return Err(anyhow!("解码 spec 内容失败: {}", e));
            }
        };

        let spec_content = match String::from_utf8(decoded) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(
                    tracking_id = snapshot.tracking_id,
                    spec_path = %spec.path,
                    spec_sha256 = %spec.sha256,
                    error = %e,
                    "创建 L1 快照失败：spec 内容不是有效的 UTF-8"
                );
                return Err(anyhow!("spec 内容不是有效的 UTF-8: {}", e));
            }
        };

        // 提取 patch 文件并在失败时记录错误日志
        let patches = match Self::extract_patches(&snapshot.files) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(
                    tracking_id = snapshot.tracking_id,
                    files_count = snapshot.files.len(),
                    error = %e,
                    "创建 L1 快照失败：提取 patch 文件出错"
                );
                return Err(e);
            }
        };

        // 提取源文件并在失败时记录错误日志
        let source_files = match Self::extract_source_files(&snapshot.files) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(
                    tracking_id = snapshot.tracking_id,
                    files_count = snapshot.files.len(),
                    error = %e,
                    "创建 L1 快照失败：提取源文件出错"
                );
                return Err(e);
            }
        };

        Ok(L1Snapshot {
            package_name,
            version,
            spec_content,
            spec_sha256: spec.sha256.clone(),
            patches,
            source_files,
            commits: snapshot.commits.clone(),
            snapshot_at: snapshot.generated_at,
        })
    }

    /// 从 RepositorySnapshot 创建 L2 快照
    pub fn create_l2_snapshot(
        package_name: String,
        snapshot: &RepositorySnapshot,
    ) -> Result<L2Snapshot> {
        // 提取 spec 信息
        let spec = snapshot
            .spec
            .as_ref()
            .ok_or_else(|| anyhow!("L2 快照缺少 spec 文件"))?;

        let version = spec
            .version
            .clone()
            .ok_or_else(|| anyhow!("无法从 spec 文件提取版本号"))?;

        // 解码 spec 内容
        use base64::Engine;
        let spec_content = String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(&spec.content_base64)
                .map_err(|e| anyhow!("解码 spec 内容失败: {}", e))?,
        )
        .map_err(|e| anyhow!("spec 内容不是有效的 UTF-8: {}", e))?;

        // 提取 patch 文件
        let patches = Self::extract_patches(&snapshot.files)?;

        // 提取源文件
        let source_files = Self::extract_source_files(&snapshot.files)?;

        // 分析定制内容
        let customizations = Self::analyze_customizations(&spec_content, &patches)?;

        Ok(L2Snapshot {
            package_name,
            version,
            spec_content,
            spec_sha256: spec.sha256.clone(),
            patches,
            source_files,
            customizations,
            commits: snapshot.commits.clone(),
            snapshot_at: snapshot.generated_at,
        })
    }

    /// 提取 patch 文件
    fn extract_patches(files: &[FileEntry]) -> Result<Vec<PatchFile>> {
        let patches = files
            .iter()
            .filter(|f| f.path.ends_with(".patch") || f.path.ends_with(".diff"))
            .map(|f| PatchFile {
                filename: f.path.split('/').next_back().unwrap_or(&f.path).to_string(),
                path: f.path.clone(),
                content_hash: f.sha256.clone(),
                size: f.size,
                applied: true, // 假设所有 patch 都已应用
            })
            .collect();

        Ok(patches)
    }

    /// 提取源文件
    fn extract_source_files(files: &[FileEntry]) -> Result<Vec<SourceFile>> {
        let source_files = files
            .iter()
            .filter(|f| {
                // 排除 patch 文件和 spec 文件
                !f.path.ends_with(".patch")
                    && !f.path.ends_with(".diff")
                    && !f.path.ends_with(".spec")
            })
            .map(|f| SourceFile {
                filename: f.path.split('/').next_back().unwrap_or(&f.path).to_string(),
                path: f.path.clone(),
                content_hash: f.sha256.clone(),
                size: f.size,
            })
            .collect();

        Ok(source_files)
    }

    /// 分析定制内容
    ///
    /// 从 spec 文件和 patch 文件中识别各种类型的定制内容：
    /// - 版本变更
    /// - 功能修改
    /// - 配置修改
    /// - 安全加固
    /// - 性能优化
    fn analyze_customizations(
        spec_content: &str,
        patches: &[PatchFile],
    ) -> Result<Vec<Customization>> {
        let mut customizations = Vec::new();

        // 1. 分析 spec 文件中的定制内容
        customizations.extend(Self::analyze_spec_customizations(spec_content)?);

        // 2. 分析 patch 文件中的定制内容
        customizations.extend(Self::analyze_patch_customizations(patches)?);

        Ok(customizations)
    }

    /// 分析 spec 文件中的定制内容
    fn analyze_spec_customizations(spec_content: &str) -> Result<Vec<Customization>> {
        let mut customizations = Vec::new();

        // 检查定制标记
        if spec_content.contains("# Custom:") || spec_content.contains("# Enterprise:") {
            customizations.push(Customization {
                customization_type: CustomizationType::Other,
                description: "spec 文件包含定制标记".to_string(),
                affected_files: vec!["*.spec".to_string()],
            });
        }

        // 检查版本变更标记
        if spec_content.contains("# Version modified") || spec_content.contains("# Custom version")
        {
            customizations.push(Customization {
                customization_type: CustomizationType::VersionChange,
                description: "spec 文件包含版本变更标记".to_string(),
                affected_files: vec!["*.spec".to_string()],
            });
        }

        // 检查配置修改标记
        let config_keywords = [
            "# Config:",
            "# Configuration:",
            "--with-",
            "--enable-",
            "--disable-",
        ];

        for keyword in &config_keywords {
            if spec_content.contains(keyword) {
                // 提取配置相关的行
                let config_lines: Vec<&str> = spec_content
                    .lines()
                    .filter(|line| line.contains(keyword))
                    .take(3) // 最多取 3 行作为示例
                    .collect();

                if !config_lines.is_empty() {
                    customizations.push(Customization {
                        customization_type: CustomizationType::ConfigurationChange,
                        description: format!("spec 文件包含配置修改: {}", config_lines.join("; ")),
                        affected_files: vec!["*.spec".to_string()],
                    });
                    break; // 只添加一次配置修改
                }
            }
        }

        // 检查性能优化标记
        let perf_keywords = [
            "# Performance:",
            "# Optimization:",
            "# Optimize",
            "-O2",
            "-O3",
            "-march=",
        ];

        for keyword in &perf_keywords {
            if spec_content.contains(keyword) {
                customizations.push(Customization {
                    customization_type: CustomizationType::PerformanceOptimization,
                    description: format!("spec 文件包含性能优化标记: {}", keyword),
                    affected_files: vec!["*.spec".to_string()],
                });
                break; // 只添加一次性能优化
            }
        }

        // 检查安全加固标记
        let security_keywords = [
            "# Security:",
            "# Hardening:",
            "-fstack-protector",
            "-D_FORTIFY_SOURCE",
            "--enable-security",
        ];

        for keyword in &security_keywords {
            if spec_content.contains(keyword) {
                customizations.push(Customization {
                    customization_type: CustomizationType::SecurityHardening,
                    description: format!("spec 文件包含安全加固标记: {}", keyword),
                    affected_files: vec!["*.spec".to_string()],
                });
                break; // 只添加一次安全加固
            }
        }

        Ok(customizations)
    }

    /// 分析 patch 文件中的定制内容
    fn analyze_patch_customizations(patches: &[PatchFile]) -> Result<Vec<Customization>> {
        let mut customizations = Vec::new();

        for patch in patches {
            let filename_lower = patch.filename.to_lowercase();

            // 1. 检查定制功能补丁
            if filename_lower.contains("custom")
                || filename_lower.contains("enterprise")
                || filename_lower.contains("internal")
                || filename_lower.contains("proprietary")
            {
                customizations.push(Customization {
                    customization_type: CustomizationType::FeatureModification,
                    description: format!("定制功能补丁: {}", patch.filename),
                    affected_files: vec![patch.path.clone()],
                });
                continue; // 已分类，跳过后续检查
            }

            // 2. 检查安全相关补丁
            if filename_lower.contains("security")
                || filename_lower.contains("hardening")
                || filename_lower.contains("cve-")
                || filename_lower.contains("vulnerability")
            {
                customizations.push(Customization {
                    customization_type: CustomizationType::SecurityHardening,
                    description: format!("安全加固补丁: {}", patch.filename),
                    affected_files: vec![patch.path.clone()],
                });
                continue;
            }

            // 3. 检查配置相关补丁
            if filename_lower.contains("config")
                || filename_lower.contains("configure")
                || filename_lower.contains("settings")
                || filename_lower.contains("options")
            {
                customizations.push(Customization {
                    customization_type: CustomizationType::ConfigurationChange,
                    description: format!("配置修改补丁: {}", patch.filename),
                    affected_files: vec![patch.path.clone()],
                });
                continue;
            }

            // 4. 检查性能优化补丁
            if filename_lower.contains("performance")
                || filename_lower.contains("optimize")
                || filename_lower.contains("optimization")
                || filename_lower.contains("perf")
                || filename_lower.contains("speed")
            {
                customizations.push(Customization {
                    customization_type: CustomizationType::PerformanceOptimization,
                    description: format!("性能优化补丁: {}", patch.filename),
                    affected_files: vec![patch.path.clone()],
                });
                continue;
            }

            // 5. 检查版本相关补丁
            if filename_lower.contains("version")
                || filename_lower.contains("upgrade")
                || filename_lower.contains("downgrade")
            {
                customizations.push(Customization {
                    customization_type: CustomizationType::VersionChange,
                    description: format!("版本变更补丁: {}", patch.filename),
                    affected_files: vec![patch.path.clone()],
                });
                continue;
            }

            // 6. 检查特定功能关键词
            let feature_keywords = [
                "feature",
                "add-",
                "enable-",
                "disable-",
                "support-",
                "implement",
            ];

            if feature_keywords
                .iter()
                .any(|kw| filename_lower.contains(kw))
            {
                customizations.push(Customization {
                    customization_type: CustomizationType::FeatureModification,
                    description: format!("功能修改补丁: {}", patch.filename),
                    affected_files: vec![patch.path.clone()],
                });
            }
        }

        Ok(customizations)
    }

    /// 执行内容对比（使用数据库获取 L2/L1 版本匹配的基线 commit）
    pub async fn compare(
        &self,
        l1_snapshot: &L1Snapshot,
        l2_snapshot: &L2Snapshot,
        db: &DatabaseConnection,
        tracking_id: i32,
    ) -> Result<L2VsL1Report> {
        // 1. 对比 spec 文件
        let spec_diff = self.compare_spec(l1_snapshot, l2_snapshot)?;

        // 2. 对比 patch 文件
        tracing::info!(
            "对比 {} 个 L1 patch 文件和 {} 个 L2 patch 文件",
            l1_snapshot.patches.len(),
            l2_snapshot.patches.len()
        );
        let patch_diff = self.compare_patches(&l1_snapshot.patches, &l2_snapshot.patches)?;

        // 3. 对比源文件
        let source_diff =
            self.compare_source_files(&l1_snapshot.source_files, &l2_snapshot.source_files)?;

        // 4. 分析定制内容
        let customization_analysis =
            self.analyze_customization_impact(&l2_snapshot.customizations)?;

        // 5. 生成同步建议
        let sync_recommendations = self.generate_sync_recommendations(
            &spec_diff,
            &patch_diff,
            &source_diff,
            &customization_analysis,
        )?;

        // 6. 检测冲突
        let conflicts = self.detect_conflicts(&spec_diff, &patch_diff, &source_diff)?;

        // 7. 对比 commit（从数据库获取 L2 最新版本并在 L1 中匹配）
        let commit_diff = self
            .compare_commit_db(l1_snapshot, l2_snapshot, db, tracking_id)
            .await?;

        Ok(L2VsL1Report {
            id: None,
            package_name: l1_snapshot.package_name.clone(),
            spec_diff,
            patch_diff,
            source_diff,
            customization_analysis,
            sync_recommendations,
            conflicts,
            commit_diff,
            created_at: Utc::now(),
        })
    }

    /// 对比 spec 文件
    fn compare_spec(&self, l1: &L1Snapshot, l2: &L2Snapshot) -> Result<SpecDiff> {
        // 检查内容是否相同
        let content_identical = l1.spec_sha256 == l2.spec_sha256;

        // 解析两个 spec 文件
        let l1_spec = SpecParser::parse(&l1.spec_content)?;
        let l2_spec = SpecParser::parse(&l2.spec_content)?;

        // 执行详细对比
        let detailed_comparison = SpecParser::compare(&l1_spec, &l2_spec);

        // 对比版本
        let version_diff = if l1.version != l2.version {
            let relationship = self.compare_version_relationship(&l1.version, &l2.version)?;
            Some(VersionDiff {
                l1_version: l1.version.clone(),
                l2_version: l2.version.clone(),
                relationship,
            })
        } else {
            None
        };

        // 生成差异摘要
        let diff_summary = if content_identical {
            "spec 文件内容完全相同".to_string()
        } else {
            detailed_comparison.summary()
        };

        // 提取关键变更
        let mut key_changes = Vec::new();

        if detailed_comparison.version_changed {
            if let Some((old_ver, new_ver)) = &detailed_comparison.version_diff {
                key_changes.push(format!("版本从 {} 变更为 {}", old_ver, new_ver));
            }
        }

        if !detailed_comparison.build_requires_added.is_empty() {
            key_changes.push(format!(
                "新增 BuildRequires: {}",
                detailed_comparison.build_requires_added.join(", ")
            ));
        }

        if !detailed_comparison.build_requires_removed.is_empty() {
            key_changes.push(format!(
                "删除 BuildRequires: {}",
                detailed_comparison.build_requires_removed.join(", ")
            ));
        }

        if !detailed_comparison.configure_options_added.is_empty() {
            key_changes.push(format!(
                "新增 configure 选项: {}",
                detailed_comparison.configure_options_added.join(" ")
            ));
        }

        if !detailed_comparison.configure_options_removed.is_empty() {
            key_changes.push(format!(
                "删除 configure 选项: {}",
                detailed_comparison.configure_options_removed.join(" ")
            ));
        }

        if detailed_comparison.sources_changed {
            key_changes.push("Source 文件列表变化".to_string());
        }

        if detailed_comparison.patches_changed {
            key_changes.push("Patch 文件列表变化".to_string());
        }

        Ok(SpecDiff {
            version_diff,
            content_identical,
            diff_summary,
            key_changes,
            detailed_comparison: Some(detailed_comparison.clone()),
            build_requires_added: detailed_comparison.build_requires_added.clone(),
            build_requires_removed: detailed_comparison.build_requires_removed.clone(),
            configure_options_added: detailed_comparison.configure_options_added.clone(),
            configure_options_removed: detailed_comparison.configure_options_removed.clone(),
        })
    }

    /// 对比版本关系
    fn compare_version_relationship(
        &self,
        l1_version: &str,
        l2_version: &str,
    ) -> Result<VersionRelationship> {
        match (
            VersionParser::parse(l1_version),
            VersionParser::parse(l2_version),
        ) {
            (Ok(v1), Ok(v2)) => {
                if v2.is_newer_than(&v1) {
                    Ok(VersionRelationship::L2Newer)
                } else if v1.is_newer_than(&v2) {
                    Ok(VersionRelationship::L2Older)
                } else {
                    Ok(VersionRelationship::Same)
                }
            }
            _ => Ok(VersionRelationship::Incomparable),
        }
    }

    /// 对比 patch 文件
    fn compare_patches(
        &self,
        l1_patches: &[PatchFile],
        l2_patches: &[PatchFile],
    ) -> Result<PatchDiff> {
        // 构建哈希映射
        let l1_map: HashMap<String, &PatchFile> =
            l1_patches.iter().map(|p| (p.filename.clone(), p)).collect();

        let l2_map: HashMap<String, &PatchFile> =
            l2_patches.iter().map(|p| (p.filename.clone(), p)).collect();

        let mut l2_added = Vec::new();
        let mut l2_removed = Vec::new();
        let mut l2_modified = Vec::new();
        let mut identical = Vec::new();

        // 检查 L2 的 patch
        for l2_patch in l2_patches {
            if let Some(l1_patch) = l1_map.get(&l2_patch.filename) {
                // 文件名相同，检查内容
                if l1_patch.content_hash == l2_patch.content_hash {
                    tracing::info!("patch {} 内容 identical", l2_patch.filename);
                    identical.push(l2_patch.clone());
                } else {
                    tracing::info!("patch {} 内容不同", l2_patch.filename);
                    l2_modified.push(PatchModification {
                        filename: l2_patch.filename.clone(),
                        l1_hash: l1_patch.content_hash.clone(),
                        l2_hash: l2_patch.content_hash.clone(),
                    });
                }
            } else {
                // L2 新增的 patch 文件
                tracing::info!("patch {} 新增", l2_patch.filename);
                l2_added.push(l2_patch.clone());
            }
        }

        // 检查 L1 有但 L2 没有的 patch 文件
        for l1_patch in l1_patches {
            if !l2_map.contains_key(&l1_patch.filename) {
                tracing::info!("patch {} 已删除", l1_patch.filename);
                l2_removed.push(l1_patch.clone());
            }
        }

        Ok(PatchDiff {
            l1_total: l1_patches.len(),
            l2_total: l2_patches.len(),
            l2_added,
            l2_removed,
            l2_modified,
            identical,
        })
    }

    /// 对比源文件
    fn compare_source_files(
        &self,
        l1_sources: &[SourceFile],
        l2_sources: &[SourceFile],
    ) -> Result<SourceDiff> {
        // 构建哈希映射
        let l1_map: HashMap<String, &SourceFile> =
            l1_sources.iter().map(|s| (s.filename.clone(), s)).collect();

        let l2_map: HashMap<String, &SourceFile> =
            l2_sources.iter().map(|s| (s.filename.clone(), s)).collect();

        let mut l2_added = Vec::new();
        let mut l2_removed = Vec::new();
        let mut l2_modified = Vec::new();

        // 检查 L2 的源文件
        for l2_source in l2_sources {
            if let Some(l1_source) = l1_map.get(&l2_source.filename) {
                // 文件名相同，检查内容
                if l1_source.content_hash != l2_source.content_hash {
                    l2_modified.push(SourceModification {
                        filename: l2_source.filename.clone(),
                        l1_hash: l1_source.content_hash.clone(),
                        l2_hash: l2_source.content_hash.clone(),
                    });
                }
            } else {
                // L2 新增的源文件
                l2_added.push(l2_source.clone());
            }
        }

        // 检查 L1 有但 L2 没有的源文件
        for l1_source in l1_sources {
            if !l2_map.contains_key(&l1_source.filename) {
                l2_removed.push(l1_source.clone());
            }
        }

        Ok(SourceDiff {
            l1_total: l1_sources.len(),
            l2_total: l2_sources.len(),
            l2_added,
            l2_removed,
            l2_modified,
        })
    }

    /// 分析定制内容影响
    ///
    /// 对定制内容进行分类统计，并生成详细的摘要报告
    fn analyze_customization_impact(
        &self,
        customizations: &[Customization],
    ) -> Result<CustomizationAnalysis> {
        let mut by_type: HashMap<String, Vec<Customization>> = HashMap::new();

        // 按类型分组
        for custom in customizations {
            let type_name = format!("{:?}", custom.customization_type);
            by_type.entry(type_name).or_default().push(custom.clone());
        }

        // 生成详细摘要
        let summary = if customizations.is_empty() {
            "未检测到定制内容".to_string()
        } else {
            let mut summary_parts = Vec::new();

            // 总体统计
            summary_parts.push(format!(
                "检测到 {} 项定制内容，包括 {} 种类型",
                customizations.len(),
                by_type.len()
            ));

            // 按类型详细说明
            let type_order = [
                ("SecurityHardening", "安全加固"),
                ("VersionChange", "版本变更"),
                ("FeatureModification", "功能修改"),
                ("ConfigurationChange", "配置修改"),
                ("PerformanceOptimization", "性能优化"),
                ("Other", "其他"),
            ];

            for (type_key, type_name_cn) in &type_order {
                if let Some(items) = by_type.get(*type_key) {
                    summary_parts.push(format!("- {}: {} 项", type_name_cn, items.len()));
                }
            }

            // 重点关注项
            let mut highlights = Vec::new();

            // 安全加固最重要
            if let Some(security_items) = by_type.get("SecurityHardening") {
                if !security_items.is_empty() {
                    highlights.push(format!(
                        "包含 {} 项安全加固，需要在同步时特别注意保留",
                        security_items.len()
                    ));
                }
            }

            // 版本变更需要注意
            if let Some(version_items) = by_type.get("VersionChange") {
                if !version_items.is_empty() {
                    highlights.push(format!(
                        "包含 {} 项版本变更，可能影响升级策略",
                        version_items.len()
                    ));
                }
            }

            // 功能修改可能导致冲突
            if let Some(feature_items) = by_type.get("FeatureModification") {
                if feature_items.len() >= 3 {
                    highlights.push(format!(
                        "包含 {} 项功能修改，同步时可能需要重新适配",
                        feature_items.len()
                    ));
                }
            }

            if !highlights.is_empty() {
                summary_parts.push("\n重点关注:".to_string());
                summary_parts.extend(highlights.into_iter().map(|h| format!("  {}", h)));
            }

            summary_parts.join("\n")
        };

        Ok(CustomizationAnalysis {
            total_customizations: customizations.len(),
            by_type,
            summary,
        })
    }

    /// 生成同步建议
    ///
    /// 识别 L1 的重要更新并生成同步建议列表，按优先级排序：
    /// 1. Critical（紧急）：安全补丁、CVE 修复
    /// 2. High（高）：版本升级、重要 Bug 修复
    /// 3. Medium（中）：一般补丁、功能更新
    /// 4. Low（低）：配置更新、文档变更
    fn generate_sync_recommendations(
        &self,
        spec_diff: &SpecDiff,
        patch_diff: &PatchDiff,
        source_diff: &SourceDiff,
        customization_analysis: &CustomizationAnalysis,
    ) -> Result<Vec<SyncRecommendation>> {
        let mut recommendations = Vec::new();

        // 1. 安全补丁建议（最高优先级 - Critical）
        recommendations.extend(self.generate_security_recommendations(patch_diff)?);

        // 2. 版本升级建议（高优先级 - High）
        recommendations.extend(self.generate_version_recommendations(spec_diff)?);

        // 3. Bug 修复补丁建议（中优先级 - Medium）
        recommendations.extend(self.generate_bugfix_recommendations(patch_diff)?);

        // 4. 功能更新建议（中优先级 - Medium）
        recommendations.extend(self.generate_feature_recommendations(patch_diff)?);

        // 5. 配置变更建议（中优先级 - Medium）
        recommendations.extend(self.generate_config_recommendations(spec_diff)?);

        // 6. 源文件更新建议（低优先级 - Low）
        recommendations.extend(self.generate_source_recommendations(source_diff)?);

        // 7. 定制内容保留建议（低优先级 - Low）
        recommendations
            .extend(self.generate_customization_recommendations(customization_analysis)?);

        // 8. 依赖变更建议（中优先级 - Medium）
        recommendations.extend(self.generate_dependency_recommendations(spec_diff)?);

        // 按优先级排序（Critical > High > Medium > Low）
        recommendations.sort_by(|a, b| a.priority.cmp(&b.priority));

        // 去重：如果有相同类型和文件的建议，保留优先级最高的
        recommendations = Self::deduplicate_recommendations(recommendations);

        // 去重后再次排序以确保顺序正确
        recommendations.sort_by(|a, b| a.priority.cmp(&b.priority));

        Ok(recommendations)
    }

    /// 生成安全补丁同步建议（Critical 优先级）
    fn generate_security_recommendations(
        &self,
        patch_diff: &PatchDiff,
    ) -> Result<Vec<SyncRecommendation>> {
        let mut recommendations = Vec::new();

        // 识别 L1 新增的安全补丁（L2 中不存在的）
        let security_patches: Vec<_> = patch_diff
            .l2_removed
            .iter()
            .filter(|p| {
                let filename_lower = p.filename.to_lowercase();
                filename_lower.contains("cve-")
                    || filename_lower.contains("security")
                    || filename_lower.contains("vulnerability")
                    || filename_lower.contains("exploit")
            })
            .collect();

        if !security_patches.is_empty() {
            // 提取 CVE 编号
            let cve_numbers: Vec<String> = security_patches
                .iter()
                .filter_map(|p| Self::extract_cve_number(&p.filename))
                .collect();

            let description = if !cve_numbers.is_empty() {
                format!(
                    "L1 新增了 {} 个安全补丁（包括 {}），强烈建议立即同步以修复安全漏洞",
                    security_patches.len(),
                    cve_numbers.join(", ")
                )
            } else {
                format!(
                    "L1 新增了 {} 个安全相关补丁，强烈建议立即同步",
                    security_patches.len()
                )
            };

            recommendations.push(SyncRecommendation {
                priority: SyncPriority::Critical,
                recommendation_type: SyncType::SecurityPatch,
                description,
                affected_files: security_patches
                    .iter()
                    .map(|p| p.filename.clone())
                    .collect(),
                estimated_effort: EffortLevel::High,
            });
        }

        Ok(recommendations)
    }

    /// 提取 CVE 编号
    ///
    /// 从文件名中提取 CVE 编号，格式为 CVE-YYYY-NNNNN
    fn extract_cve_number(filename: &str) -> Option<String> {
        let filename_upper = filename.to_uppercase();
        if let Some(start) = filename_upper.find("CVE-") {
            // CVE 格式：CVE-YYYY-NNNNN（年份4位，编号至少4位）
            let cve_part = &filename_upper[start..];

            // 查找 CVE 编号的结束位置
            // CVE 编号由 "CVE-" + 数字 + "-" + 数字组成
            let mut end = 4; // 跳过 "CVE-"
            let chars: Vec<char> = cve_part.chars().collect();

            // 跳过年份部分（4位数字）
            while end < chars.len() && chars[end].is_ascii_digit() {
                end += 1;
            }

            // 跳过中间的 "-"
            if end < chars.len() && chars[end] == '-' {
                end += 1;
            }

            // 跳过编号部分（至少4位数字）
            while end < chars.len() && chars[end].is_ascii_digit() {
                end += 1;
            }

            if end > 4 {
                Some(cve_part[..end].to_string())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// 生成版本升级建议（High 优先级）
    fn generate_version_recommendations(
        &self,
        spec_diff: &SpecDiff,
    ) -> Result<Vec<SyncRecommendation>> {
        let mut recommendations = Vec::new();

        if let Some(version_diff) = &spec_diff.version_diff {
            match version_diff.relationship {
                VersionRelationship::L2Older => {
                    // L2 版本落后，建议升级
                    recommendations.push(SyncRecommendation {
                        priority: SyncPriority::High,
                        recommendation_type: SyncType::VersionUpgrade,
                        description: format!(
                            "L2 版本 ({}) 落后于 L1 版本 ({})，建议升级以获取最新功能和修复",
                            version_diff.l2_version, version_diff.l1_version
                        ),
                        affected_files: vec!["*.spec".to_string()],
                        estimated_effort: EffortLevel::High,
                    });
                }
                VersionRelationship::L2Newer => {
                    // L2 版本领先，提示可能需要向 L1 贡献
                    recommendations.push(SyncRecommendation {
                        priority: SyncPriority::Low,
                        recommendation_type: SyncType::VersionUpgrade,
                        description: format!(
                            "L2 版本 ({}) 领先于 L1 版本 ({})，建议评估是否需要向 L1 贡献变更",
                            version_diff.l2_version, version_diff.l1_version
                        ),
                        affected_files: vec!["*.spec".to_string()],
                        estimated_effort: EffortLevel::Medium,
                    });
                }
                _ => {}
            }
        }

        Ok(recommendations)
    }

    /// 生成 Bug 修复补丁建议（Medium 优先级）
    fn generate_bugfix_recommendations(
        &self,
        patch_diff: &PatchDiff,
    ) -> Result<Vec<SyncRecommendation>> {
        let mut recommendations = Vec::new();

        // 识别 L1 新增的 Bug 修复补丁
        let bugfix_patches: Vec<_> = patch_diff
            .l2_removed
            .iter()
            .filter(|p| {
                let filename_lower = p.filename.to_lowercase();
                (filename_lower.contains("fix")
                    || filename_lower.contains("bug")
                    || filename_lower.contains("patch"))
                    // 排除安全补丁（已在 Critical 中处理）
                    && !filename_lower.contains("cve-")
                    && !filename_lower.contains("security")
                    && !filename_lower.contains("vulnerability")
            })
            .collect();

        if !bugfix_patches.is_empty() {
            recommendations.push(SyncRecommendation {
                priority: SyncPriority::Medium,
                recommendation_type: SyncType::BugFix,
                description: format!(
                    "L1 新增了 {} 个 Bug 修复补丁，建议评估并同步到 L2",
                    bugfix_patches.len()
                ),
                affected_files: bugfix_patches.iter().map(|p| p.filename.clone()).collect(),
                estimated_effort: EffortLevel::Medium,
            });
        }

        // 识别 L1 修改的补丁（可能是 Bug 修复的更新）
        if !patch_diff.l2_modified.is_empty() {
            recommendations.push(SyncRecommendation {
                priority: SyncPriority::Medium,
                recommendation_type: SyncType::BugFix,
                description: format!(
                    "L1 修改了 {} 个补丁文件，建议检查变更内容并决定是否同步",
                    patch_diff.l2_modified.len()
                ),
                affected_files: patch_diff
                    .l2_modified
                    .iter()
                    .map(|m| m.filename.clone())
                    .collect(),
                estimated_effort: EffortLevel::Medium,
            });
        }

        Ok(recommendations)
    }

    /// 生成功能更新建议（Medium 优先级）
    fn generate_feature_recommendations(
        &self,
        patch_diff: &PatchDiff,
    ) -> Result<Vec<SyncRecommendation>> {
        let mut recommendations = Vec::new();

        // 识别 L1 新增的功能补丁
        let feature_patches: Vec<_> = patch_diff
            .l2_removed
            .iter()
            .filter(|p| {
                let filename_lower = p.filename.to_lowercase();
                (filename_lower.contains("feature")
                    || filename_lower.contains("add-")
                    || filename_lower.contains("enable-")
                    || filename_lower.contains("support-")
                    || filename_lower.contains("implement"))
                    // 排除已处理的类型
                    && !filename_lower.contains("cve-")
                    && !filename_lower.contains("security")
                    && !filename_lower.contains("fix")
            })
            .collect();

        if !feature_patches.is_empty() {
            recommendations.push(SyncRecommendation {
                priority: SyncPriority::Medium,
                recommendation_type: SyncType::NewFeature,
                description: format!(
                    "L1 新增了 {} 个功能补丁，建议评估这些新功能是否适用于 L2",
                    feature_patches.len()
                ),
                affected_files: feature_patches.iter().map(|p| p.filename.clone()).collect(),
                estimated_effort: EffortLevel::Medium,
            });
        }

        Ok(recommendations)
    }

    /// 生成配置变更建议（Medium 优先级）
    fn generate_config_recommendations(
        &self,
        spec_diff: &SpecDiff,
    ) -> Result<Vec<SyncRecommendation>> {
        let mut recommendations = Vec::new();

        // BuildRequires 变更
        if !spec_diff.build_requires_added.is_empty()
            || !spec_diff.build_requires_removed.is_empty()
        {
            let mut description_parts = Vec::new();

            if !spec_diff.build_requires_added.is_empty() {
                description_parts.push(format!(
                    "新增 {} 个 BuildRequires: {}",
                    spec_diff.build_requires_added.len(),
                    spec_diff.build_requires_added.join(", ")
                ));
            }

            if !spec_diff.build_requires_removed.is_empty() {
                description_parts.push(format!(
                    "删除 {} 个 BuildRequires: {}",
                    spec_diff.build_requires_removed.len(),
                    spec_diff.build_requires_removed.join(", ")
                ));
            }

            recommendations.push(SyncRecommendation {
                priority: SyncPriority::Medium,
                recommendation_type: SyncType::ConfigUpdate,
                description: format!(
                    "L1 的 BuildRequires 发生变更：{}。建议同步以确保构建依赖正确",
                    description_parts.join("；")
                ),
                affected_files: vec!["*.spec".to_string()],
                estimated_effort: EffortLevel::Low,
            });
        }

        // configure 选项变更
        if !spec_diff.configure_options_added.is_empty()
            || !spec_diff.configure_options_removed.is_empty()
        {
            let mut description_parts = Vec::new();

            if !spec_diff.configure_options_added.is_empty() {
                description_parts.push(format!(
                    "新增选项: {}",
                    spec_diff.configure_options_added.join(" ")
                ));
            }

            if !spec_diff.configure_options_removed.is_empty() {
                description_parts.push(format!(
                    "删除选项: {}",
                    spec_diff.configure_options_removed.join(" ")
                ));
            }

            recommendations.push(SyncRecommendation {
                priority: SyncPriority::Medium,
                recommendation_type: SyncType::ConfigUpdate,
                description: format!(
                    "L1 的 configure 选项发生变更：{}。建议评估这些变更对 L2 的影响",
                    description_parts.join("；")
                ),
                affected_files: vec!["*.spec".to_string()],
                estimated_effort: EffortLevel::Medium,
            });
        }

        Ok(recommendations)
    }

    /// 生成源文件更新建议（Low 优先级）
    fn generate_source_recommendations(
        &self,
        source_diff: &SourceDiff,
    ) -> Result<Vec<SyncRecommendation>> {
        let mut recommendations = Vec::new();

        // L1 新增的源文件
        if !source_diff.l2_added.is_empty() {
            recommendations.push(SyncRecommendation {
                priority: SyncPriority::Low,
                recommendation_type: SyncType::NewFeature,
                description: format!(
                    "L1 新增了 {} 个源文件，建议检查是否需要同步",
                    source_diff.l2_added.len()
                ),
                affected_files: source_diff
                    .l2_added
                    .iter()
                    .map(|s| s.filename.clone())
                    .collect(),
                estimated_effort: EffortLevel::Low,
            });
        }

        // L1 修改的源文件
        if !source_diff.l2_modified.is_empty() {
            recommendations.push(SyncRecommendation {
                priority: SyncPriority::Low,
                recommendation_type: SyncType::ConfigUpdate,
                description: format!(
                    "L1 修改了 {} 个源文件，建议检查变更内容",
                    source_diff.l2_modified.len()
                ),
                affected_files: source_diff
                    .l2_modified
                    .iter()
                    .map(|m| m.filename.clone())
                    .collect(),
                estimated_effort: EffortLevel::Low,
            });
        }

        Ok(recommendations)
    }

    /// 生成定制内容保留建议（Low 优先级）
    fn generate_customization_recommendations(
        &self,
        customization_analysis: &CustomizationAnalysis,
    ) -> Result<Vec<SyncRecommendation>> {
        let mut recommendations = Vec::new();

        if customization_analysis.total_customizations > 0 {
            // 检查是否有安全加固定制
            let has_security = customization_analysis
                .by_type
                .get("SecurityHardening")
                .map(|items| !items.is_empty())
                .unwrap_or(false);

            let priority = if has_security {
                SyncPriority::High // 安全加固定制需要特别注意
            } else {
                SyncPriority::Low
            };

            recommendations.push(SyncRecommendation {
                priority,
                recommendation_type: SyncType::ConfigUpdate,
                description: format!(
                    "L2 包含 {} 项定制内容，同步 L1 更新时需要特别注意保留这些定制。{}",
                    customization_analysis.total_customizations,
                    if has_security {
                        "特别注意：包含安全加固定制，必须保留"
                    } else {
                        "建议在同步前备份定制内容"
                    }
                ),
                affected_files: Vec::new(),
                estimated_effort: EffortLevel::Low,
            });
        }

        Ok(recommendations)
    }

    /// 生成依赖变更建议（Medium 优先级）
    fn generate_dependency_recommendations(
        &self,
        spec_diff: &SpecDiff,
    ) -> Result<Vec<SyncRecommendation>> {
        let mut recommendations = Vec::new();

        // 检查是否有详细的 spec 对比结果
        if let Some(detailed) = &spec_diff.detailed_comparison {
            // Requires 变更（运行时依赖）
            if !detailed.requires_added.is_empty() || !detailed.requires_removed.is_empty() {
                let mut description_parts = Vec::new();

                if !detailed.requires_added.is_empty() {
                    description_parts.push(format!(
                        "新增运行时依赖: {}",
                        detailed.requires_added.join(", ")
                    ));
                }

                if !detailed.requires_removed.is_empty() {
                    description_parts.push(format!(
                        "删除运行时依赖: {}",
                        detailed.requires_removed.join(", ")
                    ));
                }

                recommendations.push(SyncRecommendation {
                    priority: SyncPriority::Medium,
                    recommendation_type: SyncType::ConfigUpdate,
                    description: format!(
                        "L1 的运行时依赖发生变更：{}。建议同步以确保运行时环境正确",
                        description_parts.join("；")
                    ),
                    affected_files: vec!["*.spec".to_string()],
                    estimated_effort: EffortLevel::Low,
                });
            }
        }

        Ok(recommendations)
    }

    /// 去重同步建议
    ///
    /// 如果有相同类型和影响文件的建议，保留优先级最高的
    fn deduplicate_recommendations(
        recommendations: Vec<SyncRecommendation>,
    ) -> Vec<SyncRecommendation> {
        use std::collections::HashSet;

        let mut seen = HashSet::new();
        let mut result = Vec::new();

        for rec in recommendations {
            // 创建唯一键：类型 + 文件列表
            let key = format!(
                "{:?}:{}",
                rec.recommendation_type,
                rec.affected_files.join(",")
            );

            if !seen.contains(&key) {
                seen.insert(key);
                result.push(rec);
            }
        }

        result
    }

    /// 检测冲突
    fn detect_conflicts(
        &self,
        spec_diff: &SpecDiff,
        patch_diff: &PatchDiff,
        source_diff: &SourceDiff,
    ) -> Result<Vec<MergeConflict>> {
        let mut conflicts = Vec::new();

        // 1. 版本冲突
        if let Some(version_diff) = &spec_diff.version_diff {
            if version_diff.relationship == VersionRelationship::L2Newer {
                conflicts.push(MergeConflict {
                    conflict_type: ConflictType::VersionConflict,
                    description: format!(
                        "L2 版本 ({}) 比 L1 版本 ({}) 更新，可能导致同步冲突",
                        version_diff.l2_version, version_diff.l1_version
                    ),
                    files: vec!["*.spec".to_string()],
                    resolution_hint: "建议先确认 L2 的版本变更原因，再决定是否回退或保持"
                        .to_string(),
                });
            }
        }

        // 2. Patch 冲突
        if !patch_diff.l2_modified.is_empty() {
            conflicts.push(MergeConflict {
                conflict_type: ConflictType::PatchConflict,
                description: format!(
                    "{} 个补丁在 L1 和 L2 中都存在但内容不同",
                    patch_diff.l2_modified.len()
                ),
                files: patch_diff
                    .l2_modified
                    .iter()
                    .map(|m| m.filename.clone())
                    .collect(),
                resolution_hint: "需要人工对比补丁内容，决定保留哪个版本或合并变更".to_string(),
            });
        }

        // 3. 文件修改冲突
        if !source_diff.l2_modified.is_empty() {
            // 检查是否有关键文件被修改
            let critical_files: Vec<_> = source_diff
                .l2_modified
                .iter()
                .filter(|m| {
                    m.filename.ends_with(".conf")
                        || m.filename.ends_with(".cfg")
                        || m.filename.ends_with(".ini")
                })
                .collect();

            if !critical_files.is_empty() {
                conflicts.push(MergeConflict {
                    conflict_type: ConflictType::FileModificationConflict,
                    description: format!(
                        "{} 个配置文件在 L1 和 L2 中都被修改",
                        critical_files.len()
                    ),
                    files: critical_files.iter().map(|m| m.filename.clone()).collect(),
                    resolution_hint: "配置文件冲突可能影响系统行为，需要仔细对比并合并".to_string(),
                });
            }
        }

        // 4. spec 文件内容冲突
        if !spec_diff.content_identical && !spec_diff.key_changes.is_empty() {
            conflicts.push(MergeConflict {
                conflict_type: ConflictType::ConfigurationConflict,
                description: "spec 文件存在关键变更，可能导致构建冲突".to_string(),
                files: vec!["*.spec".to_string()],
                resolution_hint: "建议对比 spec 文件的具体变更，确保构建配置兼容".to_string(),
            });
        }

        Ok(conflicts)
    }

    /// 对比 commit（通过数据库 version-release 匹配）
    async fn compare_commit_db(
        &self,
        l1_snapshot: &L1Snapshot,
        l2_snapshot: &L2Snapshot,
        db: &DatabaseConnection,
        tracking_id: i32,
    ) -> Result<CommitDiff> {
        use crate::entities::{l1_commit_records, l2_commit_records, prelude::*};

        let l2_latest_commit = L2CommitRecords::find()
            .filter(l2_commit_records::Column::TrackingId.eq(tracking_id))
            .order_by_desc(l2_commit_records::Column::CommittedAt)
            .one(db)
            .await?;

        let (l2_version, l2_release) = if let Some(commit) = &l2_latest_commit {
            let version = commit
                .spec_version
                .clone()
                .unwrap_or_else(|| l2_snapshot.version.clone());
            let release = commit
                .spec_release
                .clone()
                .or_else(|| Self::extract_release_from_spec(&l2_snapshot.spec_content));
            (version, release)
        } else {
            let version = l2_snapshot.version.clone();
            let release = Self::extract_release_from_spec(&l2_snapshot.spec_content);
            (version, release)
        };

        // 查询 L1 commits（时间降序）
        let l1_models = L1CommitRecords::find()
            .filter(l1_commit_records::Column::TrackingId.eq(tracking_id))
            .order_by_desc(l1_commit_records::Column::CommittedAt)
            .all(db)
            .await?;

        // 转换为 CommitEntry，保持同序
        let l1_commits: Vec<CommitEntry> =
            l1_models.iter().map(Self::model_to_commit_entry).collect();

        // 先基于数据库的 spec_version/spec_release 精确匹配
        let (base_commit, base_index) = {
            let (commit, index) =
                Self::find_base_commit_from_records(&l1_models, &l2_version, l2_release.as_deref());
            if commit.is_some() {
                (commit, index)
            } else {
                Self::find_base_commit(&l1_commits, &l2_version, l2_release.as_deref())
            }
        };

        let l1_commits_count = l1_snapshot.commits.len();
        let l2_commits_count = l2_snapshot.commits.len();

        let behind_commits = if let Some(idx) = base_index {
            l1_commits[..idx].to_vec()
        } else {
            l1_commits.clone()
        };

        let base_version_release = base_commit
            .as_ref()
            .map(|_| (l2_version.clone(), l2_release.clone()));

        Ok(CommitDiff {
            l1_commits_count,
            l2_commits_count,
            behind_commits,
            base_commit,
            base_version_release,
        })
    }

    /// 从 spec 文件内容中提取 Release 字段
    fn extract_release_from_spec(spec_content: &str) -> Option<String> {
        for line in spec_content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Release:") {
                // 提取 Release 值，可能包含宏如 %{?dist}
                let release_value = trimmed
                    .strip_prefix("Release:")
                    .map(|s| s.trim())
                    .unwrap_or("");

                // 移除常见的宏如 %{?dist}
                let cleaned = release_value
                    .replace("%{?dist}", "")
                    .replace("%{dist}", "")
                    .trim()
                    .to_string();

                if !cleaned.is_empty() {
                    return Some(cleaned);
                }
            }
        }
        None
    }

    /// 在数据库记录中查找匹配 version-release 的基线 commit（精确匹配）
    ///
    /// 返回：(基线 commit, commit 在列表中的索引)
    ///
    /// 查找策略：
    /// 1. 优先查找 spec_version 和 spec_release 完全匹配的 commit
    /// 2. 如果没找到，查找只有 spec_version 匹配的 commit
    /// 3. 如果还没找到，返回 None
    fn find_base_commit_from_records(
        models: &[crate::entities::l1_commit_records::Model],
        version: &str,
        release: Option<&str>,
    ) -> (Option<CommitEntry>, Option<usize>) {
        if let Some(rel) = release {
            for (idx, model) in models.iter().enumerate() {
                if let (Some(spec_ver), Some(spec_rel)) = (&model.spec_version, &model.spec_release)
                {
                    if spec_ver == version && spec_rel == rel {
                        return (Some(Self::model_to_commit_entry(model)), Some(idx));
                    }
                }
            }
        }
        for (idx, model) in models.iter().enumerate() {
            if let Some(spec_ver) = &model.spec_version {
                if spec_ver == version {
                    return (Some(Self::model_to_commit_entry(model)), Some(idx));
                }
            }
        }
        (None, None)
    }

    /// 将数据库 Model 转换为 CommitEntry
    fn model_to_commit_entry(model: &crate::entities::l1_commit_records::Model) -> CommitEntry {
        CommitEntry {
            sha: model.commit_sha.clone(),
            title: model
                .commit_message
                .lines()
                .next()
                .unwrap_or("")
                .to_string(),
            message: model.commit_message.clone(),
            author: model.author_name.clone(),
            authored_at: model.committed_at,
            url: Some(model.api_url.clone()),
            stats: crate::snapshot::types::ChangeStats {
                additions: model.additions,
                deletions: model.deletions,
                files_changed: model.files_changed_count,
            },
            primary_change_type: model.primary_change_type.clone(),
            cve_list: model
                .cve_list
                .as_ref()
                .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok())
                .unwrap_or_default(),
        }
    }

    /// 在 commits 列表中查找匹配 version-release 的基线 commit
    ///
    /// 返回：(基线 commit, commit 在列表中的索引)
    ///
    /// 查找策略：
    /// 1. 优先查找 commit message 中同时包含 version 和 release 的 commit
    /// 2. 如果没找到，查找只包含 version 的 commit
    /// 3. 如果还没找到，返回 None
    fn find_base_commit(
        commits: &[CommitEntry],
        version: &str,
        release: Option<&str>,
    ) -> (Option<CommitEntry>, Option<usize>) {
        // 构建搜索模式
        let version_patterns = [
            format!("Version: {}", version),
            format!("version {}", version),
            format!("v{}", version),
            version.to_string(),
        ];

        let release_patterns = release.map(|r| {
            vec![
                format!("Release: {}", r),
                format!("release {}", r),
                format!("-{}", r),
            ]
        });

        // 策略 1: 查找同时匹配 version 和 release 的 commit
        if let Some(ref rel_patterns) = release_patterns {
            for (idx, commit) in commits.iter().enumerate() {
                let message_lower = commit.message.to_lowercase();
                let title_lower = commit.title.to_lowercase();

                let has_version = version_patterns.iter().any(|pattern| {
                    message_lower.contains(&pattern.to_lowercase())
                        || title_lower.contains(&pattern.to_lowercase())
                });

                let has_release = rel_patterns.iter().any(|pattern| {
                    message_lower.contains(&pattern.to_lowercase())
                        || title_lower.contains(&pattern.to_lowercase())
                });

                if has_version && has_release {
                    tracing::info!(
                        "找到匹配 version={} release={:?} 的基线 commit: {} ({})",
                        version,
                        release,
                        commit.sha,
                        commit.title
                    );
                    return (Some(commit.clone()), Some(idx));
                }
            }
        }

        // 策略 2: 查找只匹配 version 的 commit
        for (idx, commit) in commits.iter().enumerate() {
            let message_lower = commit.message.to_lowercase();
            let title_lower = commit.title.to_lowercase();

            let has_version = version_patterns.iter().any(|pattern| {
                message_lower.contains(&pattern.to_lowercase())
                    || title_lower.contains(&pattern.to_lowercase())
            });

            if has_version {
                tracing::info!(
                    "找到匹配 version={} 的基线 commit（无 release 匹配）: {} ({})",
                    version,
                    commit.sha,
                    commit.title
                );
                return (Some(commit.clone()), Some(idx));
            }
        }

        // 策略 3: 未找到匹配的 commit
        tracing::warn!(
            "未找到匹配 version={} release={:?} 的基线 commit",
            version,
            release
        );
        (None, None)
    }
}

impl Default for L2VsL1Comparator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use sea_orm::{DatabaseBackend, MockDatabase};

    // 辅助函数：创建测试用的 RepositorySnapshot
    fn create_test_snapshot() -> RepositorySnapshot {
        use crate::snapshot::types::SnapshotOrigin;
        use crate::snapshot::types::SpecEntry;
        use base64::Engine;

        let spec_content = r#"
Name: testpkg
Version: 1.0.0
Release: 1%{?dist}
Summary: Test package

BuildRequires: gcc

%description
Test package
"#;

        let spec_base64 = base64::engine::general_purpose::STANDARD.encode(spec_content);

        RepositorySnapshot {
            tracking_id: 1,
            origin: SnapshotOrigin::L1,
            spec: Some(SpecEntry {
                path: "testpkg.spec".to_string(),
                version: Some("1.0.0".to_string()),
                release: Some("1".to_string()),
                sha256: "spec_hash".to_string(),
                content_base64: spec_base64,
            }),
            files: vec![
                FileEntry {
                    path: "test.patch".to_string(),
                    sha256: "patch_hash".to_string(),
                    size: 100,
                    is_binary: false,
                },
                FileEntry {
                    path: "source.tar.gz".to_string(),
                    sha256: "source_hash".to_string(),
                    size: 1000,
                    is_binary: false,
                },
            ],
            commits: vec![],
            generated_at: Utc::now(),
            issues: vec![],
        }
    }

    #[test]
    fn test_patch_file_equality() {
        let patch1 = PatchFile {
            filename: "test.patch".to_string(),
            path: "/path/to/test.patch".to_string(),
            content_hash: "abc123".to_string(),
            size: 1024,
            applied: true,
        };

        let patch2 = patch1.clone();
        assert_eq!(patch1, patch2);
    }

    #[test]
    fn test_source_file_equality() {
        let source1 = SourceFile {
            filename: "test.tar.gz".to_string(),
            path: "/path/to/test.tar.gz".to_string(),
            content_hash: "def456".to_string(),
            size: 2048,
        };

        let source2 = source1.clone();
        assert_eq!(source1, source2);
    }

    #[test]
    fn test_customization_type_equality() {
        assert_eq!(
            CustomizationType::VersionChange,
            CustomizationType::VersionChange
        );
        assert_ne!(
            CustomizationType::VersionChange,
            CustomizationType::SecurityHardening
        );
    }

    #[test]
    fn test_version_relationship() {
        assert_eq!(VersionRelationship::L2Newer, VersionRelationship::L2Newer);
        assert_eq!(VersionRelationship::Same, VersionRelationship::Same);
        assert_ne!(VersionRelationship::L2Newer, VersionRelationship::L2Older);
    }

    #[test]
    fn test_sync_priority_ordering() {
        assert!(SyncPriority::Critical < SyncPriority::High);
        assert!(SyncPriority::High < SyncPriority::Medium);
        assert!(SyncPriority::Medium < SyncPriority::Low);
    }

    #[test]
    fn test_sync_type_equality() {
        assert_eq!(SyncType::SecurityPatch, SyncType::SecurityPatch);
        assert_ne!(SyncType::SecurityPatch, SyncType::BugFix);
    }

    #[test]
    fn test_effort_level_equality() {
        assert_eq!(EffortLevel::Low, EffortLevel::Low);
        assert_ne!(EffortLevel::Low, EffortLevel::High);
    }

    #[test]
    fn test_conflict_type_equality() {
        assert_eq!(ConflictType::PatchConflict, ConflictType::PatchConflict);
        assert_ne!(ConflictType::PatchConflict, ConflictType::VersionConflict);
    }

    #[test]
    fn test_extract_cve_number_valid() {
        assert_eq!(
            L2VsL1Comparator::extract_cve_number("fix-CVE-2023-12345.patch"),
            Some("CVE-2023-12345".to_string())
        );

        assert_eq!(
            L2VsL1Comparator::extract_cve_number("CVE-2024-9999-security.patch"),
            Some("CVE-2024-9999".to_string())
        );

        assert_eq!(
            L2VsL1Comparator::extract_cve_number("cve-2022-1234.patch"),
            Some("CVE-2022-1234".to_string())
        );
    }

    #[test]
    fn test_extract_cve_number_invalid() {
        assert_eq!(L2VsL1Comparator::extract_cve_number("bugfix.patch"), None);
        assert_eq!(L2VsL1Comparator::extract_cve_number("CVE-.patch"), None);
        assert_eq!(L2VsL1Comparator::extract_cve_number("test.patch"), None);
    }

    #[test]
    fn test_extract_release_from_spec() {
        let spec_content = r#"
Name: mypackage
Version: 1.0.0
