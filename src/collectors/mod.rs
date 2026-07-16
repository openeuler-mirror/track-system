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

pub mod adapters;
pub mod atomgit;
pub mod error;
pub mod gitea;
pub mod gitee;
pub mod github;
pub mod gitlab;
pub mod local;
pub mod traits;
pub mod web;

pub use adapters::GitClientCollectorAdapter;
pub use atomgit::AtomGitClient;
pub use error::{ApiError, ApiResult};
pub use gitea::GiteaClient;
pub use gitee::GiteeClient;
pub use github::GitHubClient;
pub use gitlab::GitLabClient;
pub use local::LocalClient;
pub use traits::{
    Branch, CollectConfig, CollectResult, Collector, Commit, CommitMetadata, CommitStats,
    CommitsParams, FileContent, GitClient, Issue, IssueClient, IssueParams, IssueState,
    PaginationParams, PatchFile, Platform, Repository, SnapshotData, SourceFile,
};
