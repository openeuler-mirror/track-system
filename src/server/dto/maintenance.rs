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
