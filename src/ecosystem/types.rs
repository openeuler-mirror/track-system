use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemDimension {
    pub level: String,
    pub score: i32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemIndicator {
    pub key: String,
    pub label: String,
    pub value: Value,
    pub status: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemSubAssessment {
    pub level: String,
    pub confidence: String,
    pub score: i32,
    pub coverage: i32,
    pub reasons: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub indicators: Vec<EcosystemIndicator>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemAssessmentSections {
    pub source: EcosystemSubAssessment,
    pub maintenance: EcosystemSubAssessment,
    pub security: EcosystemSubAssessment,
    pub quality: EcosystemSubAssessment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemEvidenceCategory {
    pub category: String,
    pub source_type: String,
    pub source_name: String,
    pub source_url: Option<String>,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemRefreshContext {
    pub target_id: i32,
    pub target_name: String,
    pub target_type: String,
    pub rule_profile: String,
    pub refreshed_at: DateTime<Utc>,
    pub assessment_version: String,
}
