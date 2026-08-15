use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use super::types::{EcosystemAssessmentSections, EcosystemDimension};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemAssessment {
    pub report_type: String,
    pub overall_risk: String,
    pub confidence: String,
    pub summary: String,
    pub sections: EcosystemAssessmentSections,
    pub dimensions: BTreeMap<String, EcosystemDimension>,
    pub evidence_summary: Value,
    pub report_payload: Value,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemRefreshResult {
    pub target_id: i32,
    pub evidence_count: usize,
    pub report_id: i64,
    pub generated_at: DateTime<Utc>,
}
