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
    pub l2_ahead: Vec<GitCommit>,
}

/// Git 仓库客户端
pub struct GitRepositoryClient {
    repo_path: PathBuf,
}

impl GitRepositoryClient {
    /// 创建新的 Git 客户端
    pub fn new(repo_path: impl AsRef<Path>) -> Result<Self> {
        let repo_path = repo_path.as_ref().to_path_buf();

        // 验证仓库是否存在
        if !repo_path.exists() {
            return Err(anyhow!("仓库路径不存在: {}", repo_path.display()));
        }

        Ok(Self { repo_path })
    }

    /// 克隆或更新仓库
    pub fn clone_or_pull(url: &str, target_path: impl AsRef<Path>, branch: &str) -> Result<Self> {
        let target_path = target_path.as_ref();

        if target_path.exists() {
            // 仓库已存在，执行 pull
            info!(url = url, path = ?target_path, "更新现有仓库");
            let repo = Repository::open(target_path).context("打开现有仓库失败")?;

            Self::fetch_and_checkout(&repo, branch)?;
        } else {
            // 仓库不存在，执行 clone
            info!(url = url, path = ?target_path, "克隆新仓库");
            Repository::clone(url, target_path).context("克隆仓库失败")?;

            let repo = Repository::open(target_path).context("打开新克隆的仓库失败")?;

            Self::fetch_and_checkout(&repo, branch)?;
        }

        Self::new(target_path)
    }

    /// 获取指定分支的 commits
    pub fn get_commits(&self, branch: &str) -> Result<Vec<GitCommit>> {
        let repo = Repository::open(&self.repo_path).context("打开仓库失败")?;

        // 获取分支引用
        let branch_name = format!("refs/heads/{}", branch);
        let obj = repo
            .revparse_single(&branch_name)
            .or_else(|_| repo.revparse_single(&format!("remotes/origin/{}", branch)))
            .context(format!("找不到分支: {}", branch))?;

        // 创建遍历器
        let mut revwalk = repo.revwalk().context("创建 revwalk 失败")?;
        revwalk.push(obj.id()).context("设置 revwalk 起点失败")?;

        revwalk
            .set_sorting(Sort::TIME | Sort::REVERSE)
            .context("设置排序失败")?;

        let mut commits = Vec::new();

        for oid in revwalk {
            let oid = oid.context("获取 OID 失败")?;
            let commit = repo.find_commit(oid).context("查找 commit 失败")?;

            let files_changed = Self::count_files_changed(&repo, &commit)?;

            commits.push(GitCommit {
                sha: commit.id().to_string(),
                message: commit
                    .message()
                    .unwrap_or("")
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string(),
                author: commit.author().name().unwrap_or("Unknown").to_string(),
                author_email: commit.author().email().unwrap_or("").to_string(),
                committed_at: DateTime::<Utc>::from_timestamp(commit.time().seconds(), 0)
                    .unwrap_or_else(Utc::now),
                files_changed,
            });
        }

        debug!(branch = branch, count = commits.len(), "获取 commits 完成");

        Ok(commits)
    }

    /// 对比两个分支的 commits
    pub fn compare_branches(&self, l1_branch: &str, l2_branch: &str) -> Result<CommitDiff> {
        let l1_commits = self
            .get_commits(l1_branch)
            .context("获取 L1 branch commits 失败")?;
        let l2_commits = self
            .get_commits(l2_branch)
            .context("获取 L2 branch commits 失败")?;

        let diff = Self::compute_diff(&l1_commits, &l2_commits);

        info!(
            l1_branch = l1_branch,
            l2_branch = l2_branch,
            l1_ahead = diff.l1_ahead.len(),
            l2_ahead = diff.l2_ahead.len(),
            "分支对比完成"
        );

        Ok(diff)
    }

    /// 对比两个 commit 列表
    pub fn compute_diff(l1_commits: &[GitCommit], l2_commits: &[GitCommit]) -> CommitDiff {
        // 构建 L2 commit SHA 的集合用于快速查询
        let l2_shas: std::collections::HashSet<_> =
            l2_commits.iter().map(|c| c.sha.as_str()).collect();

        // 找出 L1 中有但 L2 中没有的 commits
        let l1_ahead: Vec<GitCommit> = l1_commits
            .iter()
            .filter(|c| !l2_shas.contains(c.sha.as_str()))
            .cloned()
            .collect();

        // 构建 L1 commit SHA 的集合用于快速查询
        let l1_shas: std::collections::HashSet<_> =
            l1_commits.iter().map(|c| c.sha.as_str()).collect();

        // 找出 L2 中有但 L1 中没有的 commits
        let l2_ahead: Vec<GitCommit> = l2_commits
            .iter()
            .filter(|c| !l1_shas.contains(c.sha.as_str()))
            .cloned()
            .collect();

        CommitDiff { l1_ahead, l2_ahead }
    }

    /// 私有辅助方法：获取和检出分支
    fn fetch_and_checkout(repo: &Repository, branch: &str) -> Result<()> {
        // 如果是远程仓库，先 fetch
        if let Ok(mut remote) = repo.find_remote("origin") {
            remote
                .fetch(&[branch], None, None)
                .context("Fetch 分支失败")?;
        }

        // 检出分支
        let obj = repo
            .revparse_single(&format!("refs/heads/{}", branch))
            .or_else(|_| repo.revparse_single(&format!("remotes/origin/{}", branch)))
            .context(format!("找不到分支: {}", branch))?;

        repo.set_head_detached(obj.id()).context("检出分支失败")?;

        Ok(())
    }

    /// 私有辅助方法：计算 commit 中变动的文件数
    fn count_files_changed(repo: &Repository, commit: &git2::Commit) -> Result<usize> {
        if commit.parent_count() == 0 {
            // 首个 commit，直接计算树中的文件数
            let tree = commit.tree().context("获取树失败")?;
            Ok(tree.len())
        } else {
            // 对比与父 commit 的差异
            let parent = commit.parent(0).context("获取父 commit 失败")?;
            let parent_tree = parent.tree().context("获取父树失败")?;
            let commit_tree = commit.tree().context("获取 commit 树失败")?;

            let diff = repo
                .diff_tree_to_tree(Some(&parent_tree), Some(&commit_tree), None)
                .context("生成 diff 失败")?;

            Ok(diff.deltas().len())
        }
    }
}

