pub mod activity;
pub mod atomgit;
pub mod generic_git;
pub mod gitee;
pub mod github;
pub mod gitlab;
pub mod pagure;

pub use atomgit::AtomGitMaintenanceCollector;
pub use generic_git::GenericGitMaintenanceCollector;
pub use gitee::GiteeMaintenanceCollector;
pub use github::GitHubMaintenanceCollector;
pub use gitlab::GitLabMaintenanceCollector;
pub use pagure::PagureMaintenanceCollector;
