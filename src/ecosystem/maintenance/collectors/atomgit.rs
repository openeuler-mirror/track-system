use anyhow::{anyhow, Context, Result};
use chrono::{Duration, Utc};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info, warn};

use crate::collectors::{atomgit::models::AtomGitCommit, Commit};
use crate::entities::packages;

use super::activity::{commit_timestamp, normalized_commit_identity, RepositoryActivityMetrics};

const ATOMGIT_API_BASE: &str = "https://api.atomgit.com/api/v5";
const DEFAULT_TIMEOUT_SECS: u64 = 40;
const COMMITS_PER_PAGE: u32 = 100;
const MAX_TOTAL_COUNT_PAGES: u32 = 500;
const MAX_RECENT_ACTIVITY_PAGES: u32 = 200;

#[derive(Debug, Default, Clone)]
pub struct AtomGitMaintenanceCollector;

#[derive(Debug, Deserialize)]
struct AtomGitRepositorySnapshot {
    #[serde(default)]
