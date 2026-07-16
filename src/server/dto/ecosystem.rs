use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::entities::{ecosystem_reports, ecosystem_targets};

#[derive(Debug, Clone, Deserialize)]
pub struct CreateEcosystemTargetRequest {
    pub name: String,
    pub target_type: String,
    pub platform: Option<String>,
    pub role: String,
    pub homepage_url: Option<String>,
    pub api_base_url: Option<String>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub default_branch: Option<String>,
    pub status: Option<String>,
    pub refresh_interval_hours: Option<i32>,
    pub rule_profile: String,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateEcosystemTargetRequest {
    pub name: Option<String>,
    pub target_type: Option<String>,
    pub platform: Option<String>,
    pub role: Option<String>,
    pub homepage_url: Option<String>,
    pub api_base_url: Option<String>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub default_branch: Option<String>,
    pub status: Option<String>,
    pub refresh_interval_hours: Option<i32>,
    pub rule_profile: Option<String>,
    pub metadata: Option<Value>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EcosystemTargetListQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub target_type: Option<String>,
    pub platform: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EcosystemReportListQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub target_id: Option<i32>,
    pub report_type: Option<String>,
}
