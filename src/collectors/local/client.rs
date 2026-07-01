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

//! 本地 Git 仓库采集器
//!
//! 使用 git2 库读取本地 Git 仓库的元数据

use async_trait::async_trait;
use base64::Engine;
use chrono::{TimeZone, Utc};
use git2::{Oid, Repository, Sort};
use std::path::{Path, PathBuf};

use crate::collectors::{
    error::{ApiError, ApiResult},
    traits::{
        Branch, CollectConfig, CollectResult, Collector, Commit, CommitMetadata, CommitsParams,
        FileContent, GitClient, Platform, SnapshotData,
    },
};

/// 本地 Git 仓库客户端
pub struct LocalClient {
    repo_path: PathBuf,
}

impl LocalClient {
    /// 创建新的本地客户端
    pub fn new(repo_path: impl Into<PathBuf>) -> ApiResult<Self> {
        let path = repo_path.into();

        // 验证路径存在
        if !path.exists() {
            return Err(ApiError::InvalidConfig(format!(
                "Repository path does not exist: {}",
                path.display()
            )));
        }

        // 验证是否是 Git 仓库
        Repository::open(&path)
