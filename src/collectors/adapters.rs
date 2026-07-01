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
