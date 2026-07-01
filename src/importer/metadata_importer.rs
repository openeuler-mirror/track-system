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

//! 元数据导入器
//!
//! 负责解析 track-collector 导出的 JSON 并导入到数据库

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

use crate::entities::{issues, l1_commit_records, prelude::*};

/// 导入的元数据（track-collector 格式）
#[derive(Debug, Serialize, Deserialize)]
pub struct CollectedMetadata {
    /// 平台类型（gitee/github）
    pub platform: String,
    /// 仓库所有者
    pub owner: String,
    /// 仓库名称
    pub repo: String,
    /// 分支名称
    pub branch: String,
    /// 采集时间
    pub collected_at: DateTime<Utc>,
    /// 仓库信息
    pub repository_info: Option<RepoInfo>,
    /// Commits
    pub commits: Vec<CommitInfo>,
    /// Issues
    pub issues: Vec<IssueInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepoInfo {
