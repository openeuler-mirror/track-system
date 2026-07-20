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
