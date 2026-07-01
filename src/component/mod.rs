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
    ComponentCommit {
        sha: commit.sha,
        message: commit.message,
        author_name: commit.author_name,
        author_email: commit.author_email,
        authored_at: commit.author_date,
        url: commit.html_url,
        additions: commit.stats.as_ref().map(|s| s.additions),
        deletions: commit.stats.as_ref().map(|s| s.deletions),
        total: commit.stats.as_ref().map(|s| s.total),
    }
}

fn decode_file_content(file: &FileContent) -> Result<String> {
    if file.encoding.to_lowercase() != "base64" {
        return Err(anyhow!("unsupported encoding: {}", file.encoding));
    }

    let normalized = file.content.replace('\n', "");
    let decoded = BASE64
        .decode(normalized.as_bytes())
        .map_err(|err| anyhow!("failed to decode base64 content: {}", err))?;

    String::from_utf8(decoded).map_err(|err| anyhow!("invalid utf-8 content: {}", err))
}

pub fn normalize_spec_path(repo: &str, spec: Option<&str>) -> String {
    let candidate = spec.unwrap_or(repo);
    if candidate.ends_with(".spec") {
        candidate.to_string()
    } else {
        format!("{}.spec", candidate)
    }
}

/// 将解析结果转换为公共结构
pub fn to_public_spec(name: &str, info: SpecInfo) -> ComponentSpec {
    ComponentSpec {
        name: name.to_string(),
        version: info.version,
        release: info.release,
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_file_content, normalize_spec_path};
    use crate::collectors::traits::FileContent;

    #[test]
    fn normalize_spec_name() {
        assert_eq!(normalize_spec_path("nginx", None), "nginx.spec");
        assert_eq!(
            normalize_spec_path("nginx", Some("custom.spec")),
            "custom.spec"
        );
        assert_eq!(normalize_spec_path("nginx", Some("custom")), "custom.spec");
    }

    #[test]
    fn decode_base64_content() {
        let file = FileContent {
            name: "nginx.spec".into(),
            path: "nginx.spec".into(),
            sha: "dummy".into(),
            size: 4,
            content: "VGVzdA==".into(),
            encoding: "base64".into(),
            download_url: String::new(),
        };

        let decoded = decode_file_content(&file).unwrap();
        assert_eq!(decoded, "Test");
    }

    #[test]
    fn decode_invalid_encoding() {
        let file = FileContent {
            name: "nginx.spec".into(),
            path: "nginx.spec".into(),
            sha: "dummy".into(),
            size: 4,
            content: "VGVzdA==".into(),
            encoding: "plain".into(),
            download_url: String::new(),
        };

        assert!(decode_file_content(&file).is_err());
    }
}
