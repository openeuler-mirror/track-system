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
