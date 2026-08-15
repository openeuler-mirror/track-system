use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceDimension {
    pub level: String,
    pub score: i32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceIndicator {
    pub key: String,
    pub label: String,
    pub value: Value,
    pub status: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceSubAssessment {
    pub level: String,
    pub confidence: String,
    pub score: i32,
    pub coverage: i32,
    pub reasons: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub indicators: Vec<MaintenanceIndicator>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceRefreshContext {
    pub package_id: i32,
    pub package_name: String,
    pub l0_repo_url: Option<String>,
    pub refreshed_at: DateTime<Utc>,
    pub assessment_version: String,
}
