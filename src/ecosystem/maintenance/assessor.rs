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
    }
    let (coverage, missing) = coverage_for_keys(&indicators, &required_keys);
    let mut score = 100 - (missing.len() as i32 * 8);
    let mut reasons = vec![format!("维护态势纳入 {} 个证据条目", entries.len())];

    let commit_total = indicator_i64(&indicators, "commit_total").unwrap_or(0);
    let commits_last_12_months = indicator_i64(&indicators, "commits_last_12_months").unwrap_or(0);
    let committers_last_12_months =
        indicator_i64(&indicators, "committers_last_12_months").unwrap_or(0);
    let last_commit_age_days = indicator_datetime(&indicators, "last_commit_at")
        .map(|value| (Utc::now() - value).num_days())
        .unwrap_or(365);
    let stars = indicator_i64(&indicators, "stars").unwrap_or(0);
    let forks = indicator_i64(&indicators, "forks").unwrap_or(0);

    if commit_total < 100 {
        score -= 8;
        reasons.push("L0 社区历史提交总量偏低".to_string());
    }
    if commits_last_12_months < 24 {
        score -= 18;
        reasons.push("近 12 个月提交频次不足".to_string());
    }
    if committers_last_12_months < 5 {
        score -= 16;
        reasons.push("近 12 个月活跃提交者数量偏少".to_string());
    }
    if last_commit_age_days > 90 {
        score -= 20;
        reasons.push(format!("最近一次提交距今已 {} 天", last_commit_age_days));
    }
    if social_metrics_supported && stars + forks < 50 {
        score -= 8;
        reasons.push("社区关注度与分叉规模偏低".to_string());
    }
    if social_metrics_supported && stars >= 500 {
        reasons.push("仓库具备一定社区关注度".to_string());
    }
    if !social_metrics_supported {
        reasons.push(
            "上游平台未提供统一的 star/fork 社区指标，当前仅基于 Git 历史活跃度评估".to_string(),
        );
    }
    reasons.extend(
        missing
            .iter()
            .map(|key| format!("缺少维护状态指标: {}", key)),
    );
