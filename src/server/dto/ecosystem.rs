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

#[derive(Debug, Clone, Serialize)]
pub struct EcosystemReportResponse {
    pub id: i64,
    pub target_id: i32,
    pub report_type: String,
    pub status: String,
    pub overall_risk: String,
    pub confidence: String,
    pub summary: String,
    pub dimensions: Value,
    pub evidence_summary: Option<Value>,
    pub report_payload: Value,
    pub generated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ecosystem_reports::Model> for EcosystemReportResponse {
    fn from(model: ecosystem_reports::Model) -> Self {
        Self {
            id: model.id,
            target_id: model.target_id,
            report_type: model.report_type,
            status: model.status,
            overall_risk: model.overall_risk,
            confidence: model.confidence,
            summary: model.summary,
            dimensions: model.dimensions,
            evidence_summary: model.evidence_summary,
            report_payload: model.report_payload,
            generated_at: model.generated_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}
