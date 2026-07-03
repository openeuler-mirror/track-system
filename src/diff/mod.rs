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

use anyhow::Result;
use tracing::info;

use crate::snapshot::types::{FileEntry, RepositorySnapshot, SpecEntry};

pub mod comparison_service;
pub mod git_client;
pub mod l1_vs_l0;
pub mod l2_vs_l1;
pub mod types;

pub use comparison_service::ComparisonService;
pub use git_client::GitRepositoryClient;
pub use l1_vs_l0::{
    CveAnalysis, CveInfo, L0VersionInfo, L1VersionInfo, L1VsL0Comparator, L1VsL0Report,
    PatchAnalysis, PatchInfo, UpgradableVersion, VersionTag,
};
pub use l2_vs_l1::{
    ConflictType, Customization, CustomizationAnalysis, CustomizationType, EffortLevel, L1Snapshot,
    L2Snapshot, L2VsL1Comparator, L2VsL1Report, MergeConflict, PatchDiff, PatchFile,
    PatchModification, SourceDiff, SourceFile, SourceModification, SpecDiff as L2VsL1SpecDiff,
    SyncPriority, SyncRecommendation, SyncType, VersionDiff, VersionRelationship,
};
use types::{DiffReport, FileDiff, SpecDiff, SummaryDiff};

pub fn diff_snapshots(l1: &RepositorySnapshot, l2: &RepositorySnapshot) -> Result<DiffReport> {
    info!(tracking_id = l1.tracking_id, "computing diff snapshots");

    let file_diff = diff_files(&l1.files, &l2.files);
    let spec_diff = diff_spec(l1.spec.as_ref(), l2.spec.as_ref());
