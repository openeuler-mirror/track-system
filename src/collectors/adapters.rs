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
