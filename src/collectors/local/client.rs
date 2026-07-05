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
            .map_err(|e| ApiError::InvalidConfig(format!("Not a valid Git repository: {}", e)))?;

        Ok(Self { repo_path: path })
    }

    /// 打开仓库
    fn open_repo(&self) -> ApiResult<Repository> {
        Repository::open(&self.repo_path)
            .map_err(|e| ApiError::Unknown(format!("Failed to open repository: {}", e)))
    }

    /// 获取分支的 commit SHA
    fn get_branch_commit(&self, repo: &Repository, branch: &str) -> ApiResult<Oid> {
        // 尝试多种分支引用格式
        let branch_refs = vec![
            format!("refs/heads/{}", branch),
            format!("refs/remotes/origin/{}", branch),
            branch.to_string(),
        ];

        for branch_ref in branch_refs {
            if let Ok(reference) = repo.find_reference(&branch_ref) {
                if let Ok(commit) = reference.peel_to_commit() {
                    return Ok(commit.id());
                }
            }
        }

        Err(ApiError::NotFoundError(format!(
            "Branch not found: {}",
            branch
        )))
    }

    /// 创建实现了 Collector trait 的适配器
    pub fn as_collector(self) -> impl Collector {
        use crate::collectors::adapters::GitClientCollectorAdapter;
        GitClientCollectorAdapter::new(self, Platform::Local)
    }
}

#[async_trait]
impl GitClient for LocalClient {
    async fn get_repository(
        &self,
        _owner: &str,
        _repo: &str,
    ) -> ApiResult<crate::collectors::traits::Repository> {
        let repo = self.open_repo()?;

        // 获取仓库名称（从路径）
        let repo_name = self
            .repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // 获取默认分支
        let head = repo
            .head()
            .map_err(|e| ApiError::Unknown(format!("Failed to get HEAD: {}", e)))?;

        let default_branch = head.shorthand().unwrap_or("master").to_string();

        // 获取第一个 commit 的时间作为创建时间
        let mut revwalk = repo
            .revwalk()
            .map_err(|e| ApiError::Unknown(format!("Failed to create revwalk: {}", e)))?;
        revwalk.push_head().ok();
        revwalk.set_sorting(Sort::TIME | Sort::REVERSE).ok();

        let created_at = if let Some(Ok(oid)) = revwalk.next() {
            if let Ok(commit) = repo.find_commit(oid) {
                let timestamp = commit.time().seconds();
                Utc.timestamp_opt(timestamp, 0)
                    .single()
                    .unwrap_or_else(Utc::now)
            } else {
                Utc::now()
            }
        } else {
            Utc::now()
        };

        // 获取最新 commit 的时间作为更新时间
        let updated_at = if let Ok(head_commit) = repo.head().and_then(|h| h.peel_to_commit()) {
            let timestamp = head_commit.time().seconds();
            Utc.timestamp_opt(timestamp, 0)
                .single()
                .unwrap_or_else(Utc::now)
        } else {
            Utc::now()
        };

        Ok(crate::collectors::traits::Repository {
            id: 0, // 本地仓库没有 ID
            name: repo_name.clone(),
            full_name: format!("local/{}", repo_name),
            description: None,
            html_url: format!("file://{}", self.repo_path.display()),
            default_branch,
            created_at,
            updated_at,
        })
    }

    async fn get_branches(&self, _owner: &str, _repo: &str) -> ApiResult<Vec<Branch>> {
        let repo = self.open_repo()?;
        let mut branches = Vec::new();

        // 遍历所有本地分支
        let branch_iter = repo
            .branches(Some(git2::BranchType::Local))
            .map_err(|e| ApiError::Unknown(format!("Failed to list branches: {}", e)))?;

        for (branch, _) in branch_iter.flatten() {
            if let Some(name) = branch.name().ok().flatten() {
                let reference = branch.get();
                let commit_sha = if let Ok(commit) = reference.peel_to_commit() {
                    commit.id().to_string()
                } else {
                    continue;
                };

                branches.push(Branch {
                    name: name.to_string(),
                    commit_sha,
                    protected: false, // 本地分支没有保护状态
                });
            }
        }
        Ok(branches)
    }

    async fn get_commits(
        &self,
        _owner: &str,
        _repo: &str,
        params: CommitsParams,
    ) -> ApiResult<Vec<Commit>> {
        let repo = self.open_repo()?;
        let mut commits = Vec::new();

        // 获取分支的起始 commit
        let start_oid = self.get_branch_commit(&repo, &params.branch)?;

