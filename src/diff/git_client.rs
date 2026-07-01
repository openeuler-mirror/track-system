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

//! Git 仓库客户端
//!
//! 用于本地 Git 仓库操作，支持 clone、fetch 和 commit 差异对比

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use git2::{Repository, Sort};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Git Commit 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommit {
    /// Commit SHA
    pub sha: String,
    /// Commit 消息
    pub message: String,
    /// 作者名
    pub author: String,
    /// 作者邮箱
    pub author_email: String,
    /// Commit 时间
    pub committed_at: DateTime<Utc>,
    /// 文件变动数
    pub files_changed: usize,
}

/// Commit 对比结果
#[derive(Debug, Clone)]
pub struct CommitDiff {
    /// L1 领先 commits（L1 中有但 L2 中没有）
    pub l1_ahead: Vec<GitCommit>,
    /// L2 领先 commits（L2 中有但 L1 中没有）
