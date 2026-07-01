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

        // 创建 revwalk
        let mut revwalk = repo
            .revwalk()
            .map_err(|e| ApiError::Unknown(format!("Failed to create revwalk: {}", e)))?;

        revwalk
            .push(start_oid)
            .map_err(|e| ApiError::Unknown(format!("Failed to push commit: {}", e)))?;

        revwalk.set_sorting(Sort::TIME).ok();

        // 遍历 commits
        let limit = params.per_page as usize;
        for oid_result in revwalk.take(limit) {
            let oid = oid_result
                .map_err(|e| ApiError::Unknown(format!("Failed to get commit OID: {}", e)))?;

            let commit = repo
                .find_commit(oid)
                .map_err(|e| ApiError::Unknown(format!("Failed to find commit: {}", e)))?;

            // 转换时间
            let time = commit.time();
            let timestamp = time.seconds();
            let datetime = Utc
                .timestamp_opt(timestamp, 0)
                .single()
                .unwrap_or_else(Utc::now);

            // 检查时间范围
            if let Some(since) = params.since {
                if datetime < since {
                    break;
                }
            }
            if let Some(until) = params.until {
                if datetime > until {
                    continue;
                }
            }

            // 提取作者和提交者信息
            let author = commit.author();
            let committer = commit.committer();

            let author_name = author.name().unwrap_or("Unknown").to_string();
            let author_email = author.email().unwrap_or("").to_string();
            let committer_name = committer.name().unwrap_or("Unknown").to_string();
            let committer_email = committer.email().unwrap_or("").to_string();
            let title = commit.summary().unwrap_or("").to_string();
            let message = commit.message().unwrap_or("").to_string();

            // 计算统计信息（如果有父提交）
            let stats = if commit.parent_count() > 0 {
                if let Ok(parent) = commit.parent(0) {
                    if let (Ok(parent_tree), Ok(commit_tree)) = (parent.tree(), commit.tree()) {
                        if let Ok(diff) =
                            repo.diff_tree_to_tree(Some(&parent_tree), Some(&commit_tree), None)
                        {
                            if let Ok(stats) = diff.stats() {
                                Some(crate::collectors::traits::CommitStats {
                                    additions: stats.insertions() as u32,
                                    deletions: stats.deletions() as u32,
                                    total: (stats.insertions() + stats.deletions()) as u32,
                                })
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            commits.push(Commit {
                sha: oid.to_string(),
                title,
                message,
                author_name,
                author_email,
                author_date: datetime,
                committer_name,
                committer_email,
                committer_date: datetime,
                html_url: format!("file://{}/commit/{}", self.repo_path.display(), oid),
                stats,
            });
        }

        Ok(commits)
    }

    async fn get_file_content(
        &self,
        _owner: &str,
        _repo: &str,
        path: &str,
        branch: &str,
    ) -> ApiResult<FileContent> {
        let repo = self.open_repo()?;

        // 获取分支的 commit
        let commit_oid = self.get_branch_commit(&repo, branch)?;
        let commit = repo
            .find_commit(commit_oid)
            .map_err(|e| ApiError::Unknown(format!("Failed to find commit: {}", e)))?;

        // 获取 tree
        let tree = commit
            .tree()
            .map_err(|e| ApiError::Unknown(format!("Failed to get tree: {}", e)))?;

        // 查找文件
        let entry = tree
            .get_path(Path::new(path))
            .map_err(|e| ApiError::NotFoundError(format!("File not found: {}", e)))?;

        // 获取 blob
        let object = entry
            .to_object(&repo)
            .map_err(|e| ApiError::Unknown(format!("Failed to get object: {}", e)))?;

        let blob = object
            .as_blob()
            .ok_or_else(|| ApiError::InvalidConfig("Path is not a file".to_string()))?;

        // 编码为 base64
        let content = base64::engine::general_purpose::STANDARD.encode(blob.content());

        Ok(FileContent {
            name: Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string(),
            path: path.to_string(),
            sha: entry.id().to_string(),
            size: blob.size() as u64,
            content,
            encoding: "base64".to_string(),
            download_url: format!("file://{}/{}", self.repo_path.display(), path),
        })
    }
}

#[async_trait]
impl Collector for LocalClient {
    async fn collect(&self, config: &CollectConfig) -> ApiResult<CollectResult> {
        self.validate_config(config)?;

        // 获取仓库名称
        let repo_name = self
            .repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

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
        let commits = self.get_commits("", "", params).await?;

        // 转换为 CommitMetadata
        let commit_metadata: Vec<CommitMetadata> = commits.into_iter().map(|c| c.into()).collect();

        // 尝试获取快照数据（如果是 L1/L2）
        let snapshot = {
            let repo = self.open_repo()?;
            self.collect_snapshot(&repo, &config.branch).ok()
        };

        Ok(CollectResult {
            level: "l0".to_string(), // 默认，调用者可以修改
            platform: Platform::Local.as_str().to_string(),
            owner: None,
            repo: repo_name,
            branch: config.branch.clone(),
            collected_at: Utc::now(),
            commits: commit_metadata,
            files: Vec::new(),
            spec: None,
            issues: Vec::new(),
            snapshot,
        })
    }

    fn name(&self) -> &str {
        "LocalCollector"
    }

    fn validate_config(&self, config: &CollectConfig) -> ApiResult<()> {
        if config.platform != Platform::Local {
            return Err(ApiError::InvalidConfig(
                "LocalClient requires Platform::Local".to_string(),
            ));
        }

        if config.repo_path.is_none() {
            return Err(ApiError::InvalidConfig(
                "Local platform requires repo_path".to_string(),
            ));
        }

        if config.branch.is_empty() {
            return Err(ApiError::InvalidConfig(
                "Branch name is required".to_string(),
            ));
        }

        Ok(())
    }
}

impl LocalClient {
    /// 采集快照数据（用于 L1/L2）
    fn collect_snapshot(&self, repo: &Repository, branch: &str) -> ApiResult<SnapshotData> {
        use crate::collectors::traits::{PatchFile, SourceFile};
        use sha2::{Digest, Sha256};

        // 获取分支的 commit
        let commit_oid = self.get_branch_commit(repo, branch)?;
        let commit = repo
            .find_commit(commit_oid)
            .map_err(|e| ApiError::Unknown(format!("Failed to find commit: {}", e)))?;

        // 获取 tree
        let tree = commit
            .tree()
            .map_err(|e| ApiError::Unknown(format!("Failed to get tree: {}", e)))?;

        let mut spec_path = None;
        let mut spec_content = None;
        let mut spec_content_base64 = None;
        let mut spec_version = None;
        let mut spec_release = None;
        let mut spec_sha256 = None;
        let mut patches = Vec::new();
        let mut source_files = Vec::new();
        let mut file_count = 0;

        // 遍历 tree
        tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
            file_count += 1;
            let path = format!("{}{}", root, entry.name().unwrap_or(""));

            // 获取文件内容
            if let Ok(object) = entry.to_object(repo) {
                if let Some(blob) = object.as_blob() {
                    let content = blob.content();

                    // 检查是否是 spec 文件
                    if path.ends_with(".spec") {
                        if let Ok(text) = String::from_utf8(content.to_vec()) {
                            spec_content = Some(text.clone());
                            spec_path = Some(path.clone());

                            // Convert to base64
                            spec_content_base64 =
                                Some(base64::engine::general_purpose::STANDARD.encode(content));

                            // Calculate SHA256
                            let mut hasher = Sha256::new();
                            hasher.update(content);
                            spec_sha256 = Some(format!("{:x}", hasher.finalize()));

                            // 尝试提取版本号和发行版号
                            for line in text.lines() {
                                if line.starts_with("Version:") {
                                    spec_version = Some(
                                        line.trim_start_matches("Version:").trim().to_string(),
                                    );
                                } else if line.starts_with("Release:") {
                                    spec_release = Some(
                                        line.trim_start_matches("Release:").trim().to_string(),
                                    );
                                }
                            }
                        }
                    }
                    // 检查是否是 patch 文件
                    else if path.ends_with(".patch") {
                        if let Ok(text) = String::from_utf8(content.to_vec()) {
                            let mut hasher = Sha256::new();
                            hasher.update(content);
                            let sha256 = format!("{:x}", hasher.finalize());

                            patches.push(PatchFile {
                                filename: entry.name().unwrap_or("").to_string(),
                                path: path.clone(),
                                content: text,
                                sha256,
                            });
                        }
                    }
                    // 其他源文件
                    else {
                        let mut hasher = Sha256::new();
                        hasher.update(content);
                        let sha256 = format!("{:x}", hasher.finalize());

                        source_files.push(SourceFile {
                            path: path.clone(),
                            sha256,
                            size: content.len() as u64,
                        });
                    }
                }
            }

            git2::TreeWalkResult::Ok
        })
        .ok();

        Ok(SnapshotData {
            spec_path,
            spec_content,
            spec_content_base64,
            spec_version,
            spec_release,
            spec_sha256,
            patches,
            source_files,
            file_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn setup_test_repo() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let repo = Repository::init(temp_dir.path()).unwrap();

        // Create a commit
        let path = temp_dir.path().join("test.txt");
        fs::write(&path, "test content").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("test.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        let sig = git2::Signature::now("Test User", "test@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        temp_dir
    }

    #[test]
    fn test_local_client_creation() {
        // Test invalid path
        let result = LocalClient::new("/nonexistent/path");
        assert!(result.is_err());

        // Test valid repo
        let temp_dir = setup_test_repo();
        let result = LocalClient::new(temp_dir.path());
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_repository() {
        let temp_dir = setup_test_repo();
        let client = LocalClient::new(temp_dir.path()).unwrap();

        let result = client.get_repository("owner", "repo").await;
        assert!(result.is_ok());
        let repo = result.unwrap();
        assert_eq!(
            repo.html_url,
            format!("file://{}", temp_dir.path().display())
        );
    }

    #[tokio::test]
    async fn test_get_branches() {
        let temp_dir = setup_test_repo();
        let client = LocalClient::new(temp_dir.path()).unwrap();

        let result = client.get_branches("owner", "repo").await;
        assert!(result.is_ok());
        let branches = result.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].name, "master");
    }

    #[tokio::test]
    async fn test_get_commits() {
        let temp_dir = setup_test_repo();
        let client = LocalClient::new(temp_dir.path()).unwrap();

        let params = CommitsParams::new("master");
        let result = client.get_commits("owner", "repo", params).await;
        assert!(result.is_ok());
        let commits = result.unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "Initial commit");
    }

    #[tokio::test]
    async fn test_get_file_content() {
        let temp_dir = setup_test_repo();
        let client = LocalClient::new(temp_dir.path()).unwrap();

        let result = client
            .get_file_content("owner", "repo", "test.txt", "master")
            .await;
        assert!(result.is_ok());
        let file = result.unwrap();
        assert_eq!(file.name, "test.txt");
    }

    #[tokio::test]
    async fn test_collect() {
        let temp_dir = setup_test_repo();
        let client = LocalClient::new(temp_dir.path()).unwrap();

        let config = CollectConfig {
            platform: Platform::Local,
            owner: None,
            repo: None,
            branch: "master".to_string(),
            since: None,
            until: None,
            limit: None,
            level: Some("l0".to_string()),
            repo_path: Some(temp_dir.path().to_path_buf()),
            api_url: None,
            token: None,
        };

        let result = client.collect(&config).await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.level, "l0");
        assert_eq!(res.commits.len(), 1);
    }

    #[test]
    fn test_local_client_name() {
        let temp_dir = setup_test_repo();
        let client = LocalClient::new(temp_dir.path()).unwrap();
        assert_eq!(client.name(), "LocalCollector");
    }

    #[tokio::test]
    async fn test_collect_snapshot() {
        let temp_dir = setup_test_repo();
        // Add a spec file
        let spec_path = temp_dir.path().join("test.spec");
        fs::write(&spec_path, "Name: test\nVersion: 1.0\nRelease: 1\n").unwrap();

        // Commit spec file
        let repo = Repository::open(temp_dir.path()).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("test.spec")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let sig = git2::Signature::now("Test User", "test@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Add spec", &tree, &[&parent])
            .unwrap();

        let client = LocalClient::new(temp_dir.path()).unwrap();
        let repo_ref = client.open_repo().unwrap();
        let result = client.collect_snapshot(&repo_ref, "master");

        assert!(result.is_ok());
        let snapshot = result.unwrap();
        assert!(snapshot.spec_path.is_some());
        assert_eq!(snapshot.spec_version, Some("1.0".to_string()));
    }
}
