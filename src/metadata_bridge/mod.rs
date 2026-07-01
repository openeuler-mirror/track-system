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

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::collectors::traits::GitClient;
use crate::utils::spec::SpecParser;
use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use chrono::Utc;
use regex::Regex;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, JsonValue, QueryFilter,
    QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{error, info};
use walkdir::WalkDir;

use crate::{
    entities::{
        l2_snapshots,
        prelude::{
            Issues, L1CommitRecords, L2CommitRecords, L2Snapshots, Tracking as TrackingEntity,
        },
    },
    snapshot::types::{
        ChangeStats, CommitEntry, FileEntry, IssueEntry, RepositorySnapshot, SnapshotOrigin,
        SpecEntry,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotSummary {
    pub tracking_id: i32,
    pub checksum: String,
    pub file_count: usize,
    pub spec_version: Option<String>,
    pub commit_count: usize,
    pub issue_count: usize,
}

pub async fn export_l2_snapshot<P: AsRef<Path>, Q: AsRef<Path>>(
    db: &DatabaseConnection,
    tracking_id: i32,
    repo_path: P,
    output_path: Q,
) -> Result<SnapshotSummary> {
    let tracking = TrackingEntity::find_by_id(tracking_id)
        .one(db)
        .await?
        .context("tracking configuration not found")?;

    let repo_path_buf = repo_path.as_ref().to_path_buf();
    let snapshot =
        build_repository_snapshot(db, &tracking, SnapshotOrigin::L2, Some(&repo_path_buf)).await?;
    persist_snapshot(db, &snapshot, output_path.as_ref()).await
}

pub async fn export_l1_snapshot<Q: AsRef<Path>>(
    db: &DatabaseConnection,
    tracking_id: i32,
    repo_path: Option<PathBuf>,
    output_path: Q,
) -> Result<SnapshotSummary> {
    let tracking = TrackingEntity::find_by_id(tracking_id)
        .one(db)
        .await?
        .context("tracking configuration not found")?;

    let repo_path_ref = repo_path.as_deref();
    let snapshot =
        build_repository_snapshot(db, &tracking, SnapshotOrigin::L1, repo_path_ref).await?;
    persist_snapshot(db, &snapshot, output_path.as_ref()).await
}

pub async fn import_snapshot<P: AsRef<Path>>(
    db: &DatabaseConnection,
    tracking_id: i32,
    input_path: P,
) -> Result<SnapshotSummary> {
    let tracking = TrackingEntity::find_by_id(tracking_id)
        .one(db)
        .await?
        .context("tracking configuration not found")?;

    let json = fs::read_to_string(input_path.as_ref())?;
