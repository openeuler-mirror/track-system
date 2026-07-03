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
