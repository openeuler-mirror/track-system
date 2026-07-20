use anyhow::{anyhow, Context, Result};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info};

use crate::entities::packages;

const DEFAULT_TIMEOUT_SECS: u64 = 40;

#[derive(Debug, Default, Clone)]
pub struct PagureMaintenanceCollector;

#[derive(Debug, Deserialize)]
struct PagureProjectSnapshot {
    full_url: String,
    fullname: String,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    date_modified: Option<String>,
    #[serde(default)]
    user: Option<PagureUser>,
}

#[derive(Debug, Deserialize)]
struct PagureUser {
    name: String,
}

struct PagureRepoRef {
    api_url: String,
    owner: String,
    repo: String,
    platform: &'static str,
}

impl PagureMaintenanceCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn matches_package(package: &packages::Model) -> bool {
        package
            .l0_repo_url
            .as_deref()
