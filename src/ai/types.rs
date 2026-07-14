/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FITNESS FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiAnalysisSource {
    EcosystemReport,
    MaintenanceReport,
    TrackingReport,
    AdHoc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiRiskLevel {
    Low,
    Medium,
    High,
    Critical,
    Unknown,
}

impl AiRiskLevel {
    pub fn from_report_value(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "low" | "低" => Self::Low,
            "medium" | "moderate" | "中" => Self::Medium,
            "high" | "高" => Self::High,
            "critical" | "严重" => Self::Critical,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiContext {
    pub source: AiAnalysisSource,
    pub target_name: Option<String>,
    pub target_type: Option<String>,
    pub platform: Option<String>,
    pub report_type: Option<String>,
    pub rule_risk: Option<String>,
    pub rule_confidence: Option<String>,
    pub rule_summary: Option<String>,
    pub evidence: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiAnalysisRequest {
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub max_evidence_chars: Option<usize>,
    #[serde(default)]
    pub allow_external_research: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
