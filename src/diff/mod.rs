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
    let summary = SummaryDiff {
        l1_commits: l1.commits.len(),
        l2_commits: l2.commits.len(),
        l1_issues: l1.issues.len(),
        l2_issues: l2.issues.len(),
    };

    Ok(DiffReport {
        tracking_id: l1.tracking_id,
        generated_at: l1.generated_at,
        file_diff,
        spec_diff,
        summary,
    })
}

fn diff_files(l1_files: &[FileEntry], l2_files: &[FileEntry]) -> Vec<FileDiff> {
    use std::collections::HashMap;

    let mut map_l2 = HashMap::new();
    for file in l2_files {
        map_l2.insert(&file.path, file);
    }

    let mut diffs = Vec::new();
    for file in l1_files {
        if let Some(other) = map_l2.remove(&file.path) {
            if file.sha256 != other.sha256 {
                diffs.push(FileDiff::Modified {
                    path: file.path.clone(),
                    l1_sha: file.sha256.clone(),
                    l2_sha: other.sha256.clone(),
                });
            }
        } else {
            diffs.push(FileDiff::Added {
                path: file.path.clone(),
                sha: file.sha256.clone(),
            });
        }
    }

    for remaining in map_l2.values() {
        diffs.push(FileDiff::Deleted {
            path: remaining.path.clone(),
            sha: remaining.sha256.clone(),
        });
    }

    diffs
}

fn diff_spec(l1_spec: Option<&SpecEntry>, l2_spec: Option<&SpecEntry>) -> Option<SpecDiff> {
    match (l1_spec, l2_spec) {
        (Some(lhs), Some(rhs)) => match (&lhs.version, &rhs.version) {
            (Some(v1), Some(v2)) if v1 == v2 => None,
            (Some(v1), Some(v2)) => Some(SpecDiff::Modified {
                l1_version: Some(v1.clone()),
                l2_version: Some(v2.clone()),
                l1_sha: lhs.sha256.clone(),
                l2_sha: rhs.sha256.clone(),
            }),
            _ if lhs.sha256 == rhs.sha256 => None,
            _ => Some(SpecDiff::Modified {
                l1_version: lhs.version.clone(),
                l2_version: rhs.version.clone(),
                l1_sha: lhs.sha256.clone(),
                l2_sha: rhs.sha256.clone(),
            }),
        },
        (Some(lhs), None) => Some(SpecDiff::Added {
            version: lhs.version.clone(),
            sha: lhs.sha256.clone(),
        }),
        (None, Some(rhs)) => Some(SpecDiff::Deleted {
            version: rhs.version.clone(),
            sha: rhs.sha256.clone(),
        }),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::types::{FileEntry, RepositorySnapshot, SnapshotOrigin, SpecEntry};

    #[test]
    fn test_diff_files_modified() {
        let l1_files = vec![
            FileEntry {
                path: "file1.txt".to_string(),
                sha256: "sha1".to_string(),
                size: 100,
                is_binary: false,
            },
            FileEntry {
                path: "file2.txt".to_string(),
                sha256: "sha2".to_string(),
                size: 200,
                is_binary: false,
            },
        ];
        let l2_files = vec![
            FileEntry {
                path: "file1.txt".to_string(),
                sha256: "sha1_modified".to_string(),
                size: 100,
                is_binary: false,
            },
            FileEntry {
                path: "file2.txt".to_string(),
                sha256: "sha2".to_string(),
                size: 200,
                is_binary: false,
            },
        ];

        let diff = diff_files(&l1_files, &l2_files);
        assert_eq!(diff.len(), 1);
        if let FileDiff::Modified {
            path,
            l1_sha,
            l2_sha,
        } = &diff[0]
