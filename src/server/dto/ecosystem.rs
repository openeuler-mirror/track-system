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

#[derive(Debug, Clone, Serialize)]
pub struct EcosystemTargetResponse {
    pub id: i32,
    pub name: String,
    pub target_type: String,
    pub platform: Option<String>,
    pub role: String,
    pub homepage_url: Option<String>,
    pub api_base_url: Option<String>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub default_branch: Option<String>,
    pub status: String,
    pub refresh_interval_hours: i32,
    pub rule_profile: String,
    pub metadata: Option<Value>,
    pub last_collected_at: Option<DateTime<Utc>>,
    pub last_report_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ecosystem_targets::Model> for EcosystemTargetResponse {
    fn from(model: ecosystem_targets::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            target_type: model.target_type,
            platform: model.platform,
            role: model.role,
            homepage_url: model.homepage_url,
            api_base_url: model.api_base_url,
            owner: model.owner,
            repo: model.repo,
            default_branch: model.default_branch,
            status: model.status,
            refresh_interval_hours: model.refresh_interval_hours,
            rule_profile: model.rule_profile,
            metadata: model.metadata,
            last_collected_at: model.last_collected_at,
            last_report_at: model.last_report_at,
            last_error: model.last_error,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

