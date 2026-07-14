use anyhow::{anyhow, Context, Result};
use chrono::{TimeZone, Utc};
use git2::{BranchType, Oid, Repository, Sort};
use regex::Regex;
use reqwest::Url;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};

use crate::entities::packages;
use crate::utils::version::{Version, VersionParser};

const GENERIC_GIT_CACHE_ENV: &str = "TRACK_SYSTEM_GENERIC_GIT_CACHE_DIR";
const GENERIC_GIT_CACHE_RETENTION_ENV: &str = "TRACK_SYSTEM_GENERIC_GIT_CACHE_RETENTION_ENABLED";
const GENERIC_GIT_FETCH_TIMEOUT_ENV: &str = "TRACK_SYSTEM_GENERIC_GIT_FETCH_TIMEOUT_SECS";
const GENERIC_GIT_CONNECT_TIMEOUT_ENV: &str = "TRACK_SYSTEM_GENERIC_GIT_CONNECT_TIMEOUT_SECS";
const GENERIC_GIT_IO_TIMEOUT_ENV: &str = "TRACK_SYSTEM_GENERIC_GIT_IO_TIMEOUT_SECS";
const DEFAULT_FETCH_TIMEOUT_SECS: u64 = 600;
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 15;
const DEFAULT_IO_TIMEOUT_SECS: u64 = 60;
const GIT_WAIT_POLL_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Debug, Default, Clone)]
pub struct GenericGitMaintenanceCollector;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericGitMirrorCacheSummary {
    pub repo_url: String,
    pub cache_path: PathBuf,
    pub default_branch: Option<String>,
    pub cache_retained: bool,
}

#[derive(Debug)]
struct GenericGitMetrics {
    default_branch: Option<String>,
    last_commit_at: Option<String>,
    commit_total: i64,
    commits_last_12_months: i64,
    committers_last_12_months: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GenericGitVersion {
    version: String,
    source_ref: String,
    is_stable: bool,
}

#[derive(Debug, Clone, Copy)]
struct GenericGitTimeouts {
    fetch_timeout: Duration,
    connect_timeout: Duration,
    io_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteHead {
    default_branch: Option<String>,
    head_oid: Option<Oid>,
}

impl GenericGitMaintenanceCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn matches_package(package: &packages::Model) -> bool {
        package.l0_repo_url.as_deref().is_some()
    }

    pub async fn collect(&self, package: &packages::Model) -> Result<Vec<Value>> {
        let repo_url = package
            .l0_repo_url
            .as_deref()
            .ok_or_else(|| anyhow!("package {} missing l0_repo_url", package.name))?;

        info!(
            package = package.name,
            repo_url, "开始采集通用 Git 维护指标"
        );
        let repo_url_owned = repo_url.to_string();
        let mirror_lock = cached_mirror_lock(&repo_url_owned);
        let metrics = tokio::task::spawn_blocking(move || {
            let _guard = mirror_lock
                .lock()
                .map_err(|_| anyhow!("generic git mirror lock poisoned"))?;
            collect_metrics(&repo_url_owned)
        })
        .await
        .context("join generic git collector failed")??;
        let source_url = normalize_source_url(repo_url);

        debug!(
            package = package.name,
            repo_url,
            default_branch = metrics.default_branch,
            commit_total = metrics.commit_total,
            commits_last_12_months = metrics.commits_last_12_months,
            committers_last_12_months = metrics.committers_last_12_months,
            "通用 Git 维护指标采集完成"
        );

        Ok(vec![
            json!({
                "source_type": "generic_git_repository_activity",
                "source_name": "generic_git_repository_activity",
                "source_url": source_url,
                "http_status": 200,
                "assessment_category": "maintenance",
                "assessment_subcategory": "repository_activity",
                "data": {
                    "collector": "generic_git_ls_remote_bare_fetch",
                    "repo_html_url": source_url,
                    "default_branch": metrics.default_branch,
                    "commit_total": metrics.commit_total,
                    "commits_last_12_months": metrics.commits_last_12_months,
                    "committers_last_12_months": metrics.committers_last_12_months,
                    "last_commit_at": metrics.last_commit_at,
                }
            }),
            json!({
                "source_type": "generic_git_platform_capability",
                "source_name": "generic_git_platform_capability",
                "source_url": source_url,
                "http_status": 200,
                "assessment_category": "maintenance",
                "assessment_subcategory": "platform_capability",
                "data": {
                    "social_metrics_supported": false,
                    "stars": null,
                    "forks": null,
                }
            }),
        ])
    }

    pub async fn collect_version_catalog(&self, package: &packages::Model) -> Result<Value> {
        let repo_url = package
            .l0_repo_url
            .as_deref()
            .ok_or_else(|| anyhow!("package {} missing l0_repo_url", package.name))?;

        let repo_url_owned = repo_url.to_string();
        let versions =
            tokio::task::spawn_blocking(move || collect_remote_versions(&repo_url_owned))
                .await
                .context("join generic git version catalog collector failed")??;

        Ok(build_version_catalog_evidence(repo_url, &versions))
    }
}

fn collect_metrics(repo_url: &str) -> Result<GenericGitMetrics> {
    with_synced_mirror(repo_url, |repo, _repo_path| compute_metrics(repo))
}

pub fn warm_cached_mirror(repo_url: &str) -> Result<GenericGitMirrorCacheSummary> {
    let repo_url = repo_url.trim();
    if repo_url.is_empty() {
        return Err(anyhow!("repo url is empty"));
    }

    let mirror_lock = cached_mirror_lock(repo_url);
    let _guard = mirror_lock
        .lock()
        .map_err(|_| anyhow!("generic git mirror lock poisoned"))?;

    let cache_retained = generic_git_cache_retention_enabled();
    let (cache_path, default_branch) = with_synced_mirror(repo_url, |repo, repo_path| {
        let default_branch = repo.head().ok().and_then(|head| {
            head.shorthand()
                .map(|value| value.to_string())
                .or_else(|| head.name().map(|value| value.to_string()))
        });
        Ok((repo_path.to_path_buf(), default_branch))
    })?;

    Ok(GenericGitMirrorCacheSummary {
        repo_url: normalize_source_url(repo_url),
        cache_path,
        default_branch,
        cache_retained,
    })
}

pub fn cached_mirror_path(repo_url: &str) -> PathBuf {
    cached_mirror_root().join(format!("{}.git", cached_mirror_key(repo_url)))
}

fn with_synced_mirror<T>(
    repo_url: &str,
    use_repo: impl FnOnce(&Repository, &Path) -> Result<T>,
) -> Result<T> {
    let repo_path = cached_mirror_path(repo_url);
    let cache_retained = generic_git_cache_retention_enabled();
    let result = (|| {
        let repo = sync_cached_mirror(repo_url)?;
        use_repo(&repo, &repo_path)
    })();

    if !cache_retained {
        cleanup_cached_mirror(&repo_path);
    }

    result
}

fn sync_cached_mirror(repo_url: &str) -> Result<Repository> {
    let timeouts = generic_git_timeouts();
    let cache_root = cached_mirror_root();
    fs::create_dir_all(&cache_root).context("create generic git cache root failed")?;

    let repo_path = cached_mirror_path(repo_url);
    if repo_path.exists() {
        match open_and_update_cached_mirror(&repo_path, repo_url, timeouts) {
            Ok(repo) => return Ok(repo),
            Err(error) => {
                warn!(
                    repo_url,
                    cache_path = %repo_path.display(),
                    error = %error,
                    "generic git cached mirror 已损坏，准备重建"
                );
                if repo_path.is_dir() {
                    fs::remove_dir_all(&repo_path).with_context(|| {
                        format!(
                            "remove broken cached mirror failed: {}",
                            repo_path.display()
                        )
                    })?;
                } else {
                    fs::remove_file(&repo_path).with_context(|| {
                        format!(
                            "remove broken cached mirror file failed: {}",
                            repo_path.display()
                        )
                    })?;
                }
            }
        }
    }

    create_cached_mirror(&repo_path, repo_url, timeouts)
}

fn create_cached_mirror(
    repo_path: &Path,
    repo_url: &str,
    timeouts: GenericGitTimeouts,
) -> Result<Repository> {
    debug!(
        repo_url,
        cache_path = %repo_path.display(),
        "创建 generic git 本地镜像缓存"
    );
    let repo = Repository::init_bare(repo_path)
        .with_context(|| format!("init bare mirror failed: {}", repo_path.display()))?;
    ensure_origin_remote(&repo, repo_url)?;

    let remote_head = resolve_remote_head(&repo, repo_url, timeouts)?;
    fetch_cached_mirror(&repo, repo_path, repo_url, &remote_head, timeouts)?;
    update_cached_head(&repo, remote_head.default_branch.as_deref())?;

    Ok(repo)
}

fn open_and_update_cached_mirror(
    repo_path: &Path,
    repo_url: &str,
    timeouts: GenericGitTimeouts,
) -> Result<Repository> {
    let repo = Repository::open_bare(repo_path)
        .with_context(|| format!("open cached mirror failed: {}", repo_path.display()))?;
    ensure_origin_remote(&repo, repo_url)?;

    let remote_head = resolve_remote_head(&repo, repo_url, timeouts)?;
    fetch_cached_mirror(&repo, repo_path, repo_url, &remote_head, timeouts)?;
    update_cached_head(&repo, remote_head.default_branch.as_deref())?;

    Ok(repo)
}

fn ensure_origin_remote(repo: &Repository, repo_url: &str) -> Result<()> {
    match repo.find_remote("origin") {
        Ok(remote) => {
            if remote.url() != Some(repo_url) {
                repo.remote_set_url("origin", repo_url)
                    .with_context(|| format!("update origin url failed: {}", repo_url))?;
            }
        }
        Err(_) => {
            repo.remote("origin", repo_url)
                .with_context(|| format!("create origin remote failed: {}", repo_url))?;
        }
    }

    Ok(())
}

fn resolve_remote_head(
    repo: &Repository,
    repo_url: &str,
    timeouts: GenericGitTimeouts,
) -> Result<RemoteHead> {
    let output = run_git_command_with_timeout(
        &[
            "ls-remote".to_string(),
            "--symref".to_string(),
            repo_url.to_string(),
            "HEAD".to_string(),
        ],
        timeouts.reference_timeout(),
        "connect origin remote failed",
    )?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut remote_head = parse_remote_head(&stdout);

    if remote_head.default_branch.is_none() {
        remote_head.default_branch = repo
            .head()
            .ok()
            .and_then(|head| head.name().map(|value| value.to_string()));
    }

    Ok(remote_head)
}

fn fetch_cached_mirror(
    repo: &Repository,
    repo_path: &Path,
    repo_url: &str,
    remote_head: &RemoteHead,
    timeouts: GenericGitTimeouts,
) -> Result<()> {
    if cached_head_matches_remote(repo, remote_head) {
        debug!(
            repo_url,
            default_branch = remote_head.default_branch.as_deref(),
            head_oid = remote_head.head_oid.map(|oid| oid.to_string()),
            "generic git mirror cache hit, skip fetch"
        );
        return Ok(());
    }

    let spec = remote_head
        .default_branch
        .as_deref()
        .map(|default_branch| format!("+{0}:{0}", default_branch));
    let refspec = if let Some(spec) = spec {
        spec
    } else {
        "+refs/heads/*:refs/heads/*".to_string()
    };
    let operation = format!("fetch cached mirror failed: {}", repo_url);
    let filtered_args = fetch_command_args(repo_path, &refspec, true);

    match run_git_command_with_timeout(&filtered_args, timeouts.fetch_timeout, &operation) {
        Ok(_) => Ok(()),
        Err(error) if should_retry_fetch_without_filter(&error) => {
            warn!(
                repo_url,
                error = %error,
                "generic git partial fetch unsupported, retry without object filter"
            );
            let fallback_args = fetch_command_args(repo_path, &refspec, false);
            run_git_command_with_timeout(&fallback_args, timeouts.fetch_timeout, &operation)?;
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn fetch_command_args(repo_path: &Path, refspec: &str, use_partial_filter: bool) -> Vec<String> {
    let mut args = vec![
        format!("--git-dir={}", repo_path.display()),
        "-c".to_string(),
        "protocol.version=2".to_string(),
        "fetch".to_string(),
        "--quiet".to_string(),
        "--prune".to_string(),
        "--no-tags".to_string(),
    ];
    if use_partial_filter {
        args.push("--filter=blob:none".to_string());
    }
    args.extend(["origin".to_string(), refspec.to_string()]);
    args
}

fn cached_head_matches_remote(repo: &Repository, remote_head: &RemoteHead) -> bool {
    let Some(remote_oid) = remote_head.head_oid else {
        return false;
    };

    let cached_oid = if let Some(branch) = remote_head.default_branch.as_deref() {
        repo.find_reference(branch)
            .ok()
            .and_then(|reference| reference.target())
    } else {
        repo.head().ok().and_then(|head| {
            head.target()
                .or_else(|| head.peel_to_commit().ok().map(|commit| commit.id()))
        })
    };

    cached_oid == Some(remote_oid)
}

fn should_retry_fetch_without_filter(error: &anyhow::Error) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("filter")
        || message.contains("partial clone")
        || message.contains("promisor")
        || message.contains("protocol version")
