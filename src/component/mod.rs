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

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::{DateTime, Utc};

use crate::{
    collectors::traits::{Commit, CommitsParams, FileContent, GitClient},
    spec::{parse_spec, SpecInfo},
};

/// 组件 spec 信息
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentSpec {
    pub name: String,
    pub version: String,
    pub release: String,
}

/// 组件 commit 信息
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentCommit {
    pub sha: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: DateTime<Utc>,
    pub url: String,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
    pub total: Option<u32>,
}

/// 从 Gitee 获取指定组件的 spec 信息
