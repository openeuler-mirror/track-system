use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::entities::maintenance_reports;

#[derive(Debug, Clone, Deserialize)]
pub struct MaintenanceReportListQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub package_id: Option<i32>,
    pub report_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaintenanceReportResponse {
    pub id: i64,
    pub package_id: i32,
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

impl From<maintenance_reports::Model> for MaintenanceReportResponse {
    fn from(model: maintenance_reports::Model) -> Self {
        Self {
            id: model.id,
            package_id: model.package_id,
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
