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
mod tests {
    use super::*;
    use serial_test::serial;
    use std::ffi::OsString;

    struct EnvGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }

        fn remove(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, previous }
