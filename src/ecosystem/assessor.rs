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

