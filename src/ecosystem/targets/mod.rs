use std::time::Duration;

pub mod atomgit;
pub mod github;
pub mod openeuler;

pub use atomgit::AtomGitPlatformCollector;
pub use github::GitHubPlatformCollector;
pub use openeuler::OpenEulerCommunityCollector;

pub(crate) fn configured_fetch_timeout(specific_env: &str, default_secs: u64) -> Duration {
    let secs = std::env::var(specific_env)
        .ok()
        .or_else(|| std::env::var("ECOSYSTEM_FETCH_TIMEOUT_SECS").ok())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .unwrap_or(default_secs);

    Duration::from_secs(secs)
}

#[cfg(test)]
