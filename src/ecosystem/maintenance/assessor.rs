use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::entities::packages;

use super::report::MaintenanceAssessment;
use super::types::{
    MaintenanceDimension, MaintenanceIndicator, MaintenanceRefreshContext, MaintenanceSubAssessment,
};

const ASSESSMENT_VERSION: &str = "maintenance-assessment-v1";

pub fn assess_target(
    package: &packages::Model,
    evidence_summary: Value,
    raw_evidence: &[Value],
) -> MaintenanceAssessment {
    let refreshed_at = Utc::now();
    let context = MaintenanceRefreshContext {
        package_id: package.id,
        package_name: package.name.clone(),
        l0_repo_url: package.l0_repo_url.clone(),
        refreshed_at,
        assessment_version: ASSESSMENT_VERSION.to_string(),
    };

    let section = build_maintenance_assessment(raw_evidence);
    let overall_risk = section.level.clone();
    let confidence = section.confidence.clone();

    let mut dimensions = BTreeMap::new();
    dimensions.insert(
        "maintenance_risk".to_string(),
        MaintenanceDimension {
            level: section.level.clone(),
            score: section.score,
            reasons: section.reasons.clone(),
        },
    );
    dimensions.insert(
        "coverage_risk".to_string(),
        MaintenanceDimension {
            level: level_from_score(section.coverage).to_string(),
            score: section.coverage,
            reasons: vec![format!("证据覆盖度: {}%", section.coverage)],
        },
    );
    dimensions.insert(
        "freshness_risk".to_string(),
        MaintenanceDimension {
            level: freshness_level(package.sync_interval_hours).to_string(),
            score: freshness_score(package.sync_interval_hours),
            reasons: vec![format!(
                "刷新间隔配置为 {} 小时",
                package.sync_interval_hours
            )],
        },
    );

    let summary = format!(
        "组件“{}”完成发行和维护状态评估，综合风险等级为 {}，证据置信度为 {}。",
        package.name, overall_risk, confidence
    );

    MaintenanceAssessment {
        report_type: "maintenance_profile".to_string(),
        overall_risk,
        confidence,
        summary,
        section: section.clone(),
        dimensions,
        evidence_summary,
        report_payload: json!({
            "context": context,
            "section": section,
            "evidence_catalog": build_evidence_catalog(raw_evidence),
            "raw_evidence": raw_evidence,
        }),
        generated_at: refreshed_at,
    }
}

fn build_maintenance_assessment(raw_evidence: &[Value]) -> MaintenanceSubAssessment {
    let entries = entries_by_category(raw_evidence, "maintenance");
    let indicators = collect_indicators(&entries);
    let social_metrics_supported =
        indicator_bool(&indicators, "social_metrics_supported").unwrap_or(true);
    let mut required_keys = vec![
        "commit_total",
        "commits_last_12_months",
        "committers_last_12_months",
        "last_commit_at",
    ];
    if social_metrics_supported {
        required_keys.extend(["stars", "forks"]);
