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
