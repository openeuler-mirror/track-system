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
