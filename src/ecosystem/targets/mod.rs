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
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    #[test]
    #[serial]
    fn configured_fetch_timeout_prefers_specific_env() {
        let _specific = EnvGuard::set("ECOSYSTEM_TEST_TIMEOUT_SECS", "9");
        let _global = EnvGuard::set("ECOSYSTEM_FETCH_TIMEOUT_SECS", "5");

        assert_eq!(
            configured_fetch_timeout("ECOSYSTEM_TEST_TIMEOUT_SECS", 3),
            Duration::from_secs(9)
        );
    }

    #[test]
    #[serial]
    fn configured_fetch_timeout_falls_back_to_global_and_default() {
        let _specific = EnvGuard::remove("ECOSYSTEM_TEST_TIMEOUT_SECS");
        let _global = EnvGuard::set("ECOSYSTEM_FETCH_TIMEOUT_SECS", "7");
        assert_eq!(
            configured_fetch_timeout("ECOSYSTEM_TEST_TIMEOUT_SECS", 3),
            Duration::from_secs(7)
        );

        let _invalid_global = EnvGuard::set("ECOSYSTEM_FETCH_TIMEOUT_SECS", "0");
        assert_eq!(
            configured_fetch_timeout("ECOSYSTEM_TEST_TIMEOUT_SECS", 3),
            Duration::from_secs(3)
        );
    }
}
