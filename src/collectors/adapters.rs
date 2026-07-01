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

//! Collector trait 适配器
//!
//! 为现有的 GitClient 实现提供 Collector trait 的适配

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use chrono::Utc;
use regex::Regex;
use sha2::{Digest, Sha256};
use tracing::{error, info};

use super::{
    error::ApiResult,
    gitea::GiteaClient,
    gitee::GiteeClient,
    github::GitHubClient,
    gitlab::GitLabClient,
    traits::{
        CollectConfig, CollectResult, Collector, CommitMetadata, CommitsParams, GitClient,
        Platform, SpecInfo,
    },
};

/// GitClient 到 Collector 的适配器
pub struct GitClientCollectorAdapter<T: GitClient> {
    client: T,
    platform: Platform,
}

impl<T: GitClient> GitClientCollectorAdapter<T> {
    /// 创建新的适配器
    pub fn new(client: T, platform: Platform) -> Self {
        Self { client, platform }
    }
}

// 类型别名，方便使用
pub type GitHubAdapter = GitClientCollectorAdapter<GitHubClient>;
pub type GiteeAdapter = GitClientCollectorAdapter<GiteeClient>;
pub type GiteaAdapter = GitClientCollectorAdapter<GiteaClient>;
pub type GitLabAdapter = GitClientCollectorAdapter<GitLabClient>;

/// 规范化 spec 文件路径
fn normalize_spec_path(repo: &str) -> String {
    if repo.ends_with(".spec") {
        repo.to_string()
    } else {
        format!("{}.spec", repo)
    }
}

/// 从 spec 文件内容中提取版本号
fn extract_spec_version(content: &str) -> Option<String> {
    let re = Regex::new(r"(?m)^\s*Version\s*:\s*([\w\.\-]+)").ok()?;
    re.captures(content)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// 从 spec 文件内容中提取 release 号
fn extract_spec_release(content: &str) -> Option<String> {
    let re = Regex::new(r"(?m)^\s*Release\s*:\s*([^\r\n]+)").ok()?;
    re.captures(content).and_then(|caps| caps.get(1)).map(|m| {
        let raw = m.as_str().trim();
        // 去掉常见的宏定义
        let cleaned = raw
            .replace("%{?dist}", "")
            .replace("%{?scl:", "")
            .replace("%{!?scl:", "")
            .replace("}", "")
            .trim()
            .to_string();
        cleaned
    })
}

/// 计算 SHA256 哈希
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    format!("{:x}", digest)
}

#[async_trait]
impl<T: GitClient> Collector for GitClientCollectorAdapter<T> {
    async fn collect(&self, config: &CollectConfig) -> ApiResult<CollectResult> {
        // 验证配置
        self.validate_config(config)?;

        let owner = config.owner.as_ref().unwrap();
        let repo = config.repo.as_ref().unwrap();

        // 构建 CommitsParams
        let mut params = CommitsParams::new(&config.branch);
        if let Some(since) = config.since {
            params = params.since(since);
        }
        if let Some(until) = config.until {
            params = params.until(until);
        }
        if let Some(limit) = config.limit {
            params = params.per_page(limit);
        }

        // 获取 commits
        let commits = self.client.get_commits(owner, repo, params).await?;

        // 转换为 CommitMetadata
        let commit_metadata: Vec<CommitMetadata> = commits.into_iter().map(|c| c.into()).collect();

        // 确定采集层级
        let level = config.level.as_deref().unwrap_or("l0");

        // 采集 spec 文件（仅针对 L2）
        let spec = if level == "l2" {
            self.collect_spec(owner, repo, &config.branch).await
        } else {
            None
        };

        Ok(CollectResult {
            level: level.to_string(),
            platform: self.platform.as_str().to_string(),
            owner: Some(owner.clone()),
            repo: repo.clone(),
            branch: config.branch.clone(),
            collected_at: Utc::now(),
            commits: commit_metadata,
            snapshot: None,    // L0/L1/L2 在这里不需要快照
            files: Vec::new(), // 如果需要文件列表，后续可以添加
            spec,
            issues: Vec::new(), // 如果需要 issues，后续可以添加
        })
    }

    fn name(&self) -> &str {
        match self.platform {
            Platform::GitHub => "GitHubCollector",
            Platform::GitLab => "GitLabCollector",
            Platform::Gitee => "GiteeCollector",
            Platform::Gitea => "GiteaCollector",
            Platform::Local => "LocalCollector",
        }
    }
}

impl<T: GitClient> GitClientCollectorAdapter<T> {
    /// 采集 spec 文件内容
    async fn collect_spec(&self, owner: &str, repo: &str, branch: &str) -> Option<SpecInfo> {
        let spec_path = normalize_spec_path(repo);

        info!(
            owner = %owner,
            repo = %repo,
            branch = %branch,
            spec_path = %spec_path,
            "开始获取 spec 文件"
        );

        match self
            .client
            .get_file_content(owner, repo, &spec_path, branch)
            .await
        {
            Ok(file) => {
                // 文件内容为 Base64 编码，需要解码以提取版本和计算 SHA256
                let normalized = file.content.replace('\n', "");
                let bytes = match BASE64_STANDARD.decode(normalized.as_bytes()) {
                    Ok(b) => b,
                    Err(err) => {
                        error!(
                            owner = %owner,
                            repo = %repo,
                            branch = %branch,
                            spec_path = %spec_path,
                            error = %err,
                            "Base64 解码 spec 内容失败"
                        );
                        return None;
                    }
                };

                let sha = sha256_hex(&bytes);
                let content_str = String::from_utf8(bytes.clone()).ok().unwrap_or_default();
                let version = extract_spec_version(&content_str).unwrap_or_default();
                let release = extract_spec_release(&content_str).unwrap_or_default();

                info!(
                    owner = %owner,
                    repo = %repo,
                    spec_path = %spec_path,
                    version = %version,
                    release = %release,
                    "成功获取 spec 文件"
                );

                Some(SpecInfo {
                    path: spec_path,
                    sha256: sha,
                    version,
                    release,
                    content_base64: normalized,
                })
            }
            Err(err) => {
                // 记录错误但不中断流程（spec 可能不存在）
                error!(
                    owner = %owner,
                    repo = %repo,
                    branch = %branch,
                    spec_path = %spec_path,
                    error = %err,
                    "获取 spec 文件失败"
                );
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::error::ApiResult;
    use crate::collectors::traits::{Commit, CommitsParams, FileContent, GitClient};
    use async_trait::async_trait;
    use mockall::mock;

    mock! {
        pub GitClient {}
        #[async_trait]
        impl GitClient for GitClient {
            async fn get_repository(&self, owner: &str, repo: &str) -> ApiResult<crate::collectors::traits::Repository>;
            async fn get_branches(&self, owner: &str, repo: &str) -> ApiResult<Vec<crate::collectors::traits::Branch>>;
            async fn get_commits(&self, owner: &str, repo: &str, params: CommitsParams) -> ApiResult<Vec<Commit>>;
            async fn get_file_content(&self, owner: &str, repo: &str, path: &str, branch: &str) -> ApiResult<FileContent>;
        }
    }

    #[test]
    fn test_normalize_spec_path() {
        assert_eq!(normalize_spec_path("my-repo"), "my-repo.spec");
        assert_eq!(normalize_spec_path("my-repo.spec"), "my-repo.spec");
    }

    #[test]
    fn test_extract_spec_version() {
        let content = "Name: test\nVersion: 1.2.3\nRelease: 1\n";
        assert_eq!(extract_spec_version(content), Some("1.2.3".to_string()));

        let content_with_spaces = "Name: test\nVersion :  1.2.3  \nRelease: 1\n";
        assert_eq!(
            extract_spec_version(content_with_spaces),
            Some("1.2.3".to_string())
        );

        let content_missing = "Name: test\nRelease: 1\n";
        assert_eq!(extract_spec_version(content_missing), None);
    }

    #[test]
    fn test_extract_spec_release() {
        let content = "Name: test\nVersion: 1.2.3\nRelease: 1%{?dist}\n";
