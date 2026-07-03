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
