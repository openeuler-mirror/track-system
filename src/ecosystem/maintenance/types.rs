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

