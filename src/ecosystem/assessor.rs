use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::ecosystem::report::EcosystemAssessment;
use crate::ecosystem::types::{
    EcosystemAssessmentSections, EcosystemDimension, EcosystemIndicator, EcosystemRefreshContext,
    EcosystemSubAssessment,
};
use crate::entities::ecosystem_targets;

const ASSESSMENT_VERSION: &str = "ecosystem-assessment-v2";

pub fn assess_target(
    target: &ecosystem_targets::Model,
    evidence_summary: Value,
    raw_evidence: &[Value],
) -> EcosystemAssessment {
    let refreshed_at = Utc::now();
    let context = EcosystemRefreshContext {
        target_id: target.id,
        target_name: target.name.clone(),
        target_type: target.target_type.clone(),
        rule_profile: target.rule_profile.clone(),
        refreshed_at,
        assessment_version: ASSESSMENT_VERSION.to_string(),
    };

    let source = build_source_assessment(raw_evidence);
    let maintenance = build_maintenance_assessment(raw_evidence);
    let security = build_security_assessment(raw_evidence);
    let quality = build_quality_assessment(raw_evidence);

    let sections = EcosystemAssessmentSections {
        source: source.clone(),
        maintenance: maintenance.clone(),
        security: security.clone(),
        quality: quality.clone(),
    };

    let overall_risk = worst_level([
        source.level.as_str(),
        maintenance.level.as_str(),
        security.level.as_str(),
        quality.level.as_str(),
    ]);
    let confidence = aggregate_confidence([
        source.confidence.as_str(),
        maintenance.confidence.as_str(),
        security.confidence.as_str(),
        quality.confidence.as_str(),
    ]);

    let coverage_score =
        (source.coverage + maintenance.coverage + security.coverage + quality.coverage) / 4;
    let mut dimensions = BTreeMap::new();
    dimensions.insert(
        "source_risk".to_string(),
        EcosystemDimension {
            level: source.level.clone(),
            score: source.score,
            reasons: source.reasons.clone(),
        },
    );
    dimensions.insert(
        "maintenance_risk".to_string(),
        EcosystemDimension {
            level: maintenance.level.clone(),
            score: maintenance.score,
            reasons: maintenance.reasons.clone(),
        },
    );
    dimensions.insert(
        "security_risk".to_string(),
        EcosystemDimension {
            level: security.level.clone(),
            score: security.score,
            reasons: security.reasons.clone(),
        },
    );
    dimensions.insert(
        "quality_risk".to_string(),
        EcosystemDimension {
            level: quality.level.clone(),
            score: quality.score,
            reasons: quality.reasons.clone(),
        },
    );
    dimensions.insert(
        "coverage_risk".to_string(),
        EcosystemDimension {
            level: level_from_score(coverage_score).to_string(),
            score: coverage_score,
            reasons: vec![
                format!("规则画像: {}", target.rule_profile),
                format!("证据覆盖度: {}%", coverage_score),
            ],
        },
    );
    dimensions.insert(
        "freshness_risk".to_string(),
        EcosystemDimension {
            level: freshness_level(target.refresh_interval_hours).to_string(),
            score: freshness_score(target.refresh_interval_hours),
            reasons: vec![format!(
                "刷新间隔配置为 {} 小时",
                target.refresh_interval_hours
            )],
        },
    );

    let summary = format!(
        "目标“{}”完成生态评估，来源/维护/安全/质量四个子模块的综合风险等级为 {}，证据置信度为 {}。",
        target.name, overall_risk, confidence
    );

    EcosystemAssessment {
        report_type: "ecosystem_profile".to_string(),
        overall_risk,
        confidence,
        summary,
        sections: sections.clone(),
        dimensions,
        evidence_summary,
        report_payload: json!({
            "context": context,
            "sections": sections,
            "evidence_catalog": build_evidence_catalog(raw_evidence),
            "raw_evidence": raw_evidence,
        }),
        generated_at: refreshed_at,
    }
}

fn build_source_assessment(raw_evidence: &[Value]) -> EcosystemSubAssessment {
    let entries = entries_by_category(raw_evidence, "source");
    let indicators = collect_indicators(&entries);
    let required_keys = [
        "organization_structure",
        "foundation_status",
        "version_lifecycle",
        "license_policy",
        "cla_policy",
        "basic_info",
        "trade_controls",
        "ip_policy",
        "government_takedown_policy",
        "top_contributors",
        "foundation_list",
        "donor_countries",
    ];
    let (coverage, missing) = coverage_for_keys(&indicators, &required_keys);
    let mut score = 100 - (missing.len() as i32 * 6);
    let mut reasons = vec![format!("来源评估已覆盖 {} 个证据条目", entries.len())];

    if contains_risk_phrase(&entries, &["trade_controls", "government_takedown_policy"]) {
        score -= 12;
        reasons.push("平台侧存在贸易管制或政府下架等外部治理约束".to_string());
    }
    if missing.iter().any(|key| key == "foundation_status") {
        score -= 10;
        reasons.push("缺少基金会归属信息，难以判断治理稳定性".to_string());
    }
    if missing.iter().any(|key| key == "top_contributors") {
        score -= 10;
        reasons.push("缺少组件社区核心贡献者列表".to_string());
    }
    if missing
        .iter()
        .any(|key| key == "license_policy" || key == "cla_policy")
    {
        score -= 8;
        reasons.push("许可证或 CLA 信息不完整".to_string());
    }
    if missing.is_empty() {
        reasons.push("社区组织、平台治理与组件社区画像信息较完整".to_string());
    }
    reasons.extend(
        missing
            .iter()
            .map(|key| format!("缺少关键来源指标: {}", key)),
    );
    finalize_assessment(
        score,
        coverage,
        reasons,
        indicators,
        collect_evidence_refs(&entries),
    )
}

fn build_maintenance_assessment(raw_evidence: &[Value]) -> EcosystemSubAssessment {
    let entries = entries_by_category(raw_evidence, "maintenance");
    let indicators = collect_indicators(&entries);
    let required_keys = [
        "commit_total",
        "commits_last_12_months",
        "committers_last_12_months",
        "last_commit_at",
        "stars",
        "forks",
    ];
    let (coverage, missing) = coverage_for_keys(&indicators, &required_keys);
    let mut score = 100 - (missing.len() as i32 * 8);
    let mut reasons = vec![format!("维护态势纳入 {} 个证据条目", entries.len())];

    let commits_last_12_months = indicator_i64(&indicators, "commits_last_12_months").unwrap_or(0);
    let committers_last_12_months =
        indicator_i64(&indicators, "committers_last_12_months").unwrap_or(0);
    let commit_total = indicator_i64(&indicators, "commit_total").unwrap_or(0);
    let stars = indicator_i64(&indicators, "stars").unwrap_or(0);
    let last_commit_age_days = indicator_datetime(&indicators, "last_commit_at")
        .map(|value| (Utc::now() - value).num_days())
        .unwrap_or(365);

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
    if stars + indicator_i64(&indicators, "forks").unwrap_or(0) < 50 {
        score -= 8;
        reasons.push("社区关注度与分叉规模偏低".to_string());
    }
    if stars >= 500 {
        reasons.push("仓库具备一定社区关注度".to_string());
    }
    reasons.extend(
        missing
            .iter()
            .map(|key| format!("缺少维护状态指标: {}", key)),
    );
    finalize_assessment(
        score,
        coverage,
        reasons,
        indicators,
        collect_evidence_refs(&entries),
    )
}

