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
pub async fn fetch_component_spec<C: GitClient + ?Sized>(
    client: &C,
    owner: &str,
    repo: &str,
    branch: &str,
    spec_path: &str,
) -> Result<ComponentSpec> {
    let file = client
        .get_file_content(owner, repo, spec_path, branch)
        .await
        .map_err(|err| anyhow!("failed to fetch spec file: {}", err))?;

    let content = decode_file_content(&file)?;
    let spec = parse_spec(&content);

    Ok(ComponentSpec {
        name: repo.to_string(),
        version: spec.version,
        release: spec.release,
    })
}

/// 获取仓库 commit 列表
pub async fn fetch_component_commits<C: GitClient + ?Sized>(
    client: &C,
    owner: &str,
    repo: &str,
    branch: &str,
    page: u32,
    per_page: u32,
) -> Result<Vec<ComponentCommit>> {
    let params = CommitsParams::new(branch.to_string())
        .page(page)
        .per_page(per_page);

    let commits = client
        .get_commits(owner, repo, params)
        .await
        .map_err(|err| anyhow!("failed to fetch commits: {}", err))?;

    Ok(commits.into_iter().map(from_commit).collect())
}

fn from_commit(commit: Commit) -> ComponentCommit {
