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

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 文件级元数据条目
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub is_binary: bool,
}

/// spec 元数据
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpecEntry {
    pub path: String,
    pub sha256: String,
    pub version: Option<String>,
    pub release: Option<String>,
    pub content_base64: String,
}

/// 变更统计信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ChangeStats {
    pub additions: i32,
    pub deletions: i32,
    pub files_changed: i32,
}

/// Commit 信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitEntry {
    pub sha: String,
    pub title: String,
    pub message: String,
    pub author: String,
    pub authored_at: DateTime<Utc>,
    pub url: Option<String>,
    pub stats: ChangeStats,
    pub primary_change_type: Option<String>,
    pub cve_list: Vec<String>,
}

/// Issue 信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueEntry {
    pub number: String,
    pub title: String,
    pub state: String,
    pub author: String,
    pub labels: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

/// 通用仓库快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositorySnapshot {
    pub tracking_id: i32,
    pub generated_at: DateTime<Utc>,
    pub origin: SnapshotOrigin,
    pub files: Vec<FileEntry>,
    pub spec: Option<SpecEntry>,
    pub commits: Vec<CommitEntry>,
    pub issues: Vec<IssueEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SnapshotOrigin {
    L1,
    L2,
    Unknown,
}

impl RepositorySnapshot {
    pub fn new(tracking_id: i32, origin: SnapshotOrigin) -> Self {
        Self {
            tracking_id,
            generated_at: Utc::now(),
            origin,
            files: Vec::new(),
            spec: None,
            commits: Vec::new(),
            issues: Vec::new(),
        }
    }
}
