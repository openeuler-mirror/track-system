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
