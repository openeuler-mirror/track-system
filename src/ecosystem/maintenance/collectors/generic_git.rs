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
}

fn parse_remote_head(output: &str) -> RemoteHead {
    let mut remote_head = RemoteHead {
        default_branch: None,
        head_oid: None,
    };

    for line in output.lines() {
        if remote_head.default_branch.is_none() {
            remote_head.default_branch = parse_default_branch_ref(line);
        }
        if remote_head.head_oid.is_none() {
            remote_head.head_oid = parse_head_oid(line);
        }
    }

    remote_head
}

fn update_cached_head(repo: &Repository, default_branch: Option<&str>) -> Result<()> {
    if let Some(default_branch) = default_branch {
        repo.set_head(default_branch)
            .with_context(|| format!("set cached mirror HEAD failed: {}", default_branch))?;
        return Ok(());
    }

    let mut branches = repo
        .branches(Some(BranchType::Local))
        .context("list cached mirror branches failed")?;
    if let Some(branch) = branches.next() {
        let (branch, _) = branch.context("read cached mirror branch failed")?;
        if let Some(name) = branch.get().name() {
            repo.set_head(name)
                .with_context(|| format!("set fallback cached HEAD failed: {}", name))?;
        }
    }

    Ok(())
}

fn cached_mirror_root() -> PathBuf {
    if let Some(path) = std::env::var_os(GENERIC_GIT_CACHE_ENV).filter(|value| !value.is_empty()) {
        return PathBuf::from(path);
    }

    dirs::cache_dir()
        .unwrap_or_else(|| std::env::temp_dir().join("track-system-cache"))
        .join("track-system")
        .join("generic-git-mirrors")
}

fn cleanup_cached_mirror(repo_path: &Path) {
    let cleanup_result = if repo_path.is_dir() {
        fs::remove_dir_all(repo_path)
    } else if repo_path.exists() {
        fs::remove_file(repo_path)
    } else {
        Ok(())
    };

    if let Err(error) = cleanup_result {
        warn!(
            cache_path = %repo_path.display(),
            error = %error,
            "generic git ephemeral cache cleanup failed"
        );
    } else {
        debug!(
            cache_path = %repo_path.display(),
            "generic git ephemeral cache cleaned"
        );
    }
}

fn cached_mirror_key(repo_url: &str) -> String {
    let normalized = normalize_source_url(repo_url);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    let hint = normalized
        .rsplit('/')
        .next()
        .unwrap_or("repo")
        .trim_end_matches(".git")
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() {
                value.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(48)
        .collect::<String>();
    let hint = if hint.is_empty() {
        "repo"
    } else {
        hint.as_str()
    };
    format!("{}-{}", &digest[..16], hint)
}

fn cached_mirror_lock(repo_url: &str) -> Arc<Mutex<()>> {
    static MIRROR_LOCKS: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();

    let key = cached_mirror_key(repo_url);
    let locks = MIRROR_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = locks
        .lock()
        .expect("generic git mirror lock table poisoned");
    guard
        .entry(key)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

impl GenericGitTimeouts {
    fn reference_timeout(self) -> Duration {
        self.connect_timeout.saturating_add(self.io_timeout)
    }
}

fn generic_git_timeouts() -> GenericGitTimeouts {
    GenericGitTimeouts {
        fetch_timeout: configured_timeout(
            GENERIC_GIT_FETCH_TIMEOUT_ENV,
            DEFAULT_FETCH_TIMEOUT_SECS,
        ),
        connect_timeout: configured_timeout(
            GENERIC_GIT_CONNECT_TIMEOUT_ENV,
            DEFAULT_CONNECT_TIMEOUT_SECS,
        ),
        io_timeout: configured_timeout(GENERIC_GIT_IO_TIMEOUT_ENV, DEFAULT_IO_TIMEOUT_SECS),
    }
}

fn generic_git_cache_retention_enabled() -> bool {
    std::env::var(GENERIC_GIT_CACHE_RETENTION_ENV)
        .ok()
        .as_deref()
        .map(parse_bool_env)
        .unwrap_or(false)
}

fn parse_bool_env(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "on" | "enable" | "enabled"
    )
}

fn configured_timeout(env_key: &str, default_secs: u64) -> Duration {
    let secs = std::env::var(env_key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .unwrap_or(default_secs);
    Duration::from_secs(secs)
}

fn parse_default_branch_ref(line: &str) -> Option<String> {
    let line = line.trim();
    if !line.starts_with("ref: ") {
        return None;
    }

    let (reference, head) = line[5..].split_once('\t')?;
    if head == "HEAD" && !reference.trim().is_empty() {
        Some(reference.trim().to_string())
    } else {
        None
    }
}

fn parse_head_oid(line: &str) -> Option<Oid> {
    let (oid, reference) = line.trim().split_once('\t')?;
    if reference == "HEAD" {
        Oid::from_str(oid.trim()).ok()
    } else {
        None
    }
}

fn collect_remote_versions(repo_url: &str) -> Result<Vec<GenericGitVersion>> {
    let output = run_git_command_with_timeout(
        &[
            "ls-remote".to_string(),
            "--tags".to_string(),
            "--refs".to_string(),
            repo_url.to_string(),
        ],
        generic_git_timeouts().reference_timeout(),
        "list remote tags failed",
    )?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_remote_tag_versions(&stdout))
}

fn build_version_catalog_evidence(repo_url: &str, versions: &[GenericGitVersion]) -> Value {
    let source_url = normalize_source_url(repo_url);
    let version_entries = versions
        .iter()
        .map(|version| {
            json!({
                "version": version.version,
                "source_ref": version.source_ref,
                "is_stable": version.is_stable,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "source_type": "generic_git_version_catalog",
        "source_name": "generic_git_version_catalog",
        "source_url": source_url,
        "http_status": 200,
        "assessment_category": "maintenance",
        "assessment_subcategory": "version_catalog",
        "data": {
            "collector": "generic_git_ls_remote_tags",
            "repo_html_url": source_url,
            "latest_version": latest_version_from_generic_versions(versions, false),
            "latest_stable": latest_version_from_generic_versions(versions, true)
                .or_else(|| latest_version_from_generic_versions(versions, false)),
            "versions": version_entries,
        }
    })
}

fn parse_remote_tag_versions(output: &str) -> Vec<GenericGitVersion> {
    let mut by_version: HashMap<String, GenericGitVersion> = HashMap::new();

    for line in output.lines() {
        let Some((_oid, reference)) = line.trim().split_once('\t') else {
            continue;
        };
        let Some(version) = normalize_tag_version(reference) else {
            continue;
        };
        let is_stable = VersionParser::parse(&version)
            .map(|parsed| parsed.is_stable())
            .unwrap_or(false);

        by_version
            .entry(version.clone())
            .or_insert_with(|| GenericGitVersion {
                version,
                source_ref: reference.trim().trim_end_matches("^{}").to_string(),
                is_stable,
            });
    }

    let mut versions = by_version.into_values().collect::<Vec<_>>();
    versions.sort_by(|left, right| compare_version_text(&left.version, &right.version));
    versions
}

fn normalize_tag_version(reference: &str) -> Option<String> {
    let tag = reference
        .trim()
        .trim_end_matches("^{}")
        .strip_prefix("refs/tags/")
        .unwrap_or(reference.trim())
        .trim();
    if tag.is_empty() || is_non_release_tag(tag) {
        return None;
    }

    static VERSION_RE: OnceLock<Regex> = OnceLock::new();
    let version_re = VERSION_RE.get_or_init(|| {
        Regex::new(
            r"(?ix)
            (?:^|[^0-9])
            v?
            (?P<core>\d+(?:[._]\d+){1,3})
            (?:
                [-._]?
                (?P<pre>alpha|beta|rc|pre)
                [-._]?
                (?P<pre_num>\d*)
            )?
            ",
        )
        .expect("generic git tag version regex")
    });

    let captures = version_re.captures(tag)?;
    let mut candidate = captures
        .name("core")
        .map(|matched| matched.as_str().replace('_', "."))?;
    if let Some(pre) = captures.name("pre") {
        candidate.push('-');
        candidate.push_str(&pre.as_str().to_ascii_lowercase());
        if let Some(num) = captures
            .name("pre_num")
            .filter(|num| !num.as_str().is_empty())
        {
            candidate.push_str(num.as_str());
        }
    }

    if is_probable_date_tag_version(&candidate) {
        return None;
    }
    VersionParser::parse(&candidate).ok()?;

    Some(candidate)
}

fn is_non_release_tag(tag: &str) -> bool {
    let lower = tag.to_ascii_lowercase();
    ["snapshot", "nightly", "test", "tmp", "debug", "wip", "dev"]
        .iter()
        .any(|needle| lower.contains(needle))
}

fn is_probable_date_tag_version(version: &str) -> bool {
    let parts = version
        .split(['.', '-'])
        .take(3)
        .filter_map(|part| part.parse::<u32>().ok())
        .collect::<Vec<_>>();

    matches!(parts.as_slice(), [1900..=2100, 1..=12, 1..=31])
}

fn latest_version_from_generic_versions(
    versions: &[GenericGitVersion],
    stable_only: bool,
) -> Option<String> {
    versions
        .iter()
        .filter(|version| !stable_only || version.is_stable)
        .filter_map(|version| {
            VersionParser::parse(&version.version)
                .ok()
                .map(|parsed| (parsed, version.version.clone()))
        })
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(_, raw)| raw)
}

fn compare_version_text(left: &str, right: &str) -> std::cmp::Ordering {
    let left = VersionParser::parse(left).unwrap_or_else(|_| Version::new(0, 0, 0));
    let right = VersionParser::parse(right).unwrap_or_else(|_| Version::new(0, 0, 0));
    left.cmp(&right)
}

fn run_git_command_with_timeout(
    args: &[String],
    timeout: Duration,
    operation: &str,
) -> Result<Output> {
    let mut child = Command::new("git")
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn git command failed: {}", operation))?;

    let started_at = Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .with_context(|| format!("wait git command failed: {}", operation))?
        {
            let stdout = read_child_pipe(&mut child.stdout)?;
            let stderr = read_child_pipe(&mut child.stderr)?;
            let output = Output {
                status,
                stdout,
                stderr,
            };

            if output.status.success() {
                return Ok(output);
            }

            let detail = command_output_message(&output);
            return Err(anyhow!("{}: {}", operation, detail));
        }

        if started_at.elapsed() >= timeout {
            child.kill().ok();
            let _ = child.wait();
            let stderr = read_child_pipe(&mut child.stderr).unwrap_or_default();
            let detail = String::from_utf8_lossy(&stderr).trim().to_string();
            if detail.is_empty() {
                return Err(anyhow!(
                    "{} timed out after {}s",
                    operation,
                    timeout.as_secs()
                ));
            }

            return Err(anyhow!(
                "{} timed out after {}s: {}",
                operation,
                timeout.as_secs(),
                detail
            ));
        }

        thread::sleep(GIT_WAIT_POLL_INTERVAL);
    }
}

fn read_child_pipe(pipe: &mut Option<impl Read>) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    if let Some(mut pipe) = pipe.take() {
        pipe.read_to_end(&mut buf)
            .context("read git command output failed")?;
    }
    Ok(buf)
}

fn command_output_message(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return stdout;
    }

    output
        .status
        .code()
        .map(|code| format!("git exited with status {}", code))
        .unwrap_or_else(|| "git exited without a status code".to_string())
}

fn compute_metrics(repo: &Repository) -> Result<GenericGitMetrics> {
    let head = repo.head().context("read repository HEAD failed")?;
    let head_oid = head
        .target()
        .or_else(|| head.peel_to_commit().ok().map(|commit| commit.id()))
        .ok_or_else(|| anyhow!("resolve HEAD target failed"))?;
    let default_branch = head.shorthand().map(|s| s.to_string());

    let mut walk = repo.revwalk().context("create revwalk failed")?;
    walk.set_sorting(Sort::TIME)
        .context("set revwalk sorting failed")?;
    walk.push(head_oid).context("push HEAD to revwalk failed")?;

    let since_ts = (Utc::now() - chrono::Duration::days(365)).timestamp();
    let mut commit_total = 0_i64;
    let mut commits_last_12_months = 0_i64;
    let mut unique_committers = std::collections::BTreeSet::new();
    let mut last_commit_at = None;

    for oid in walk {
        let oid = oid.context("iterate revwalk failed")?;
        let commit = repo.find_commit(oid).context("find commit failed")?;
        commit_total += 1;

        if last_commit_at.is_none() {
            let ts = commit.time().seconds();
            last_commit_at = Utc.timestamp_opt(ts, 0).single().map(|dt| dt.to_rfc3339());
        }

        let ts = commit.time().seconds();
        if ts >= since_ts {
            commits_last_12_months += 1;
            let author = commit.author();
            if let Some(email) = author.email().filter(|email| !email.trim().is_empty()) {
                unique_committers.insert(email.to_ascii_lowercase());
            } else if let Some(name) = author.name().filter(|name| !name.trim().is_empty()) {
                unique_committers.insert(format!("name:{}", name));
            } else {
                unique_committers.insert(format!("oid:{}", oid));
            }
        }
    }

    Ok(GenericGitMetrics {
        default_branch,
        last_commit_at,
        commit_total,
        commits_last_12_months,
        committers_last_12_months: unique_committers.len() as i64,
    })
}

fn normalize_source_url(repo_url: &str) -> String {
    if Url::parse(repo_url).is_ok() {
        repo_url
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .to_string()
    } else {
        repo_url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Oid, Signature};
    use serial_test::serial;
    use std::ffi::OsString;
    use tempfile::tempdir;

    struct EnvGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
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

    fn commit_file(
        repo: &Repository,
        path: &std::path::Path,
        message: &str,
        sig: &Signature<'_>,
        parent: Option<Oid>,
    ) -> Oid {
        std::fs::write(path, message).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        match parent {
            Some(parent_oid) => {
                let parent_commit = repo.find_commit(parent_oid).unwrap();
                repo.commit(Some("HEAD"), sig, sig, message, &tree, &[&parent_commit])
                    .unwrap()
            }
            None => repo
                .commit(Some("HEAD"), sig, sig, message, &tree, &[])
                .unwrap(),
        }
    }

    #[test]
    fn compute_metrics_from_local_repo() {
        let dir = tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = Signature::now("Test User", "test@example.com").unwrap();
        let file_path = dir.path().join("file.txt");
        let first = commit_file(&repo, &file_path, "first", &sig, None);
        let _second = commit_file(&repo, &file_path, "second", &sig, Some(first));

        let metrics = compute_metrics(&repo).unwrap();
        assert_eq!(metrics.commit_total, 2);
        assert_eq!(metrics.commits_last_12_months, 2);
        assert_eq!(metrics.committers_last_12_months, 1);
        assert!(metrics.last_commit_at.is_some());
        assert!(metrics.default_branch.is_some());
    }

    #[tokio::test]
    #[serial]
    async fn collect_from_local_repo_builds_activity_and_platform_evidence() {
        let source_dir = tempdir().unwrap();
        let source_repo = Repository::init(source_dir.path()).unwrap();
        let sig = Signature::now("Test User", "test@example.com").unwrap();
        let file_path = source_dir.path().join("file.txt");
        let first = commit_file(&source_repo, &file_path, "first", &sig, None);
        commit_file(&source_repo, &file_path, "second", &sig, Some(first));

        let cache_dir = tempdir().unwrap();
        let _cache_dir = EnvGuard::set(GENERIC_GIT_CACHE_ENV, cache_dir.path());
        let _retention = EnvGuard::set(GENERIC_GIT_CACHE_RETENTION_ENV, "false");
        let repo_url = source_dir.path().display().to_string();
        let package = packages::Model {
            id: 1,
            name: "openssl".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: Some(repo_url.clone()),
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let evidence = GenericGitMaintenanceCollector::new()
            .collect(&package)
            .await
            .unwrap();

        assert_eq!(evidence.len(), 2);
        assert_eq!(
            evidence[0]["source_type"],
            "generic_git_repository_activity"
        );
        assert_eq!(evidence[0]["data"]["commit_total"], 2);
        assert_eq!(evidence[0]["data"]["committers_last_12_months"], 1);
        assert_eq!(evidence[1]["data"]["social_metrics_supported"], false);
        assert_eq!(evidence[1]["data"]["stars"], Value::Null);
    }

    #[test]
    fn cached_mirror_key_is_stable_and_readable() {
        let repo_url = "https://git.savannah.gnu.org/git/bash.git";
        let lhs = cached_mirror_key(repo_url);
        let rhs = cached_mirror_key(repo_url);

        assert_eq!(lhs, rhs);
        assert!(lhs.ends_with("-bash"));
        assert!(cached_mirror_path(repo_url)
            .display()
            .to_string()
            .ends_with(&format!("{}.git", lhs)));
    }

    #[test]
    fn parse_default_branch_ref_from_ls_remote_output() {
        assert_eq!(
            parse_default_branch_ref("ref: refs/heads/main\tHEAD"),
            Some("refs/heads/main".to_string())
        );
        assert_eq!(parse_default_branch_ref("deadbeef\tHEAD"), None);
    }

    #[test]
    fn parse_remote_head_from_ls_remote_output() {
        let oid = "0123456789abcdef0123456789abcdef01234567";
        let output = format!("ref: refs/heads/main\tHEAD\n{oid}\tHEAD\n");

        let remote_head = parse_remote_head(&output);

        assert_eq!(
            remote_head.default_branch.as_deref(),
            Some("refs/heads/main")
        );
        assert_eq!(remote_head.head_oid.unwrap().to_string(), oid);
    }

    #[test]
    fn parse_remote_tag_versions_normalizes_release_tags() {
        let output = "\
0123456789abcdef0123456789abcdef01234567\trefs/tags/release-1_2_0
1123456789abcdef0123456789abcdef01234567\trefs/tags/v1.10.0
2123456789abcdef0123456789abcdef01234567\trefs/tags/2.0.0.rc1
3123456789abcdef0123456789abcdef01234567\trefs/tags/nightly-3.0.0
4123456789abcdef0123456789abcdef01234567\trefs/tags/2024.05.10
";

        let versions = parse_remote_tag_versions(output);
        let parsed = versions
            .iter()
            .map(|version| version.version.as_str())
            .collect::<Vec<_>>();

        assert_eq!(parsed, vec!["1.2.0", "1.10.0", "2.0.0-rc1"]);
        assert_eq!(
            latest_version_from_generic_versions(&versions, false).as_deref(),
            Some("2.0.0-rc1")
        );
        assert_eq!(
            latest_version_from_generic_versions(&versions, true).as_deref(),
            Some("1.10.0")
        );
    }

    #[tokio::test]
    async fn collect_version_catalog_reads_local_repo_tags() {
        let source_dir = tempdir().unwrap();
        let source_repo = Repository::init(source_dir.path()).unwrap();
        let sig = Signature::now("Test User", "test@example.com").unwrap();
        let file_path = source_dir.path().join("file.txt");
        let oid = commit_file(&source_repo, &file_path, "first", &sig, None);
        let commit = source_repo.find_commit(oid).unwrap();
        source_repo
            .tag("v1.0.0", commit.as_object(), &sig, "release", false)
            .unwrap();
        source_repo
            .tag("v1.1.0-rc1", commit.as_object(), &sig, "rc", false)
            .unwrap();
        source_repo
            .tag("nightly-2.0.0", commit.as_object(), &sig, "nightly", false)
            .unwrap();

        let package = packages::Model {
            id: 1,
            name: "openssl".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: Some(source_dir.path().display().to_string()),
            description: None,
            created_at: Utc::now(),
