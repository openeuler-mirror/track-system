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

fn build_security_assessment(raw_evidence: &[Value]) -> EcosystemSubAssessment {
    let entries = entries_by_category(raw_evidence, "security");
    let indicators = collect_indicators(&entries);
    let required_keys = [
        "has_security_policy",
        "cve_fix_commits_last_12_months",
        "cve_linked_issues_last_12_months",
        "median_cve_fix_days",
        "open_cve_backlog",
    ];
    let (coverage, missing) = coverage_for_keys(&indicators, &required_keys);
    let mut score = 100 - (missing.len() as i32 * 8);
    let mut reasons = vec![format!("安全评估已纳入 {} 个证据条目", entries.len())];

    let has_security_policy = indicator_bool(&indicators, "has_security_policy").unwrap_or(false);
    let cve_fix_commits = indicator_i64(&indicators, "cve_fix_commits_last_12_months").unwrap_or(0);
    let cve_linked_issues =
        indicator_i64(&indicators, "cve_linked_issues_last_12_months").unwrap_or(0);
    let median_fix_days = indicator_i64(&indicators, "median_cve_fix_days").unwrap_or(120);
    let open_cve_backlog = indicator_i64(&indicators, "open_cve_backlog").unwrap_or(10);

    if !has_security_policy {
        score -= 20;
        reasons.push("未发现明确的安全披露或响应策略".to_string());
    }
    if cve_fix_commits == 0 || cve_linked_issues == 0 {
        score -= 18;
        reasons.push("近 12 个月 CVE 与 commit/issue 的联动证据不足".to_string());
    }
    if median_fix_days > 30 {
        score -= 18;
        reasons.push(format!(
            "CVE 修复中位时长为 {} 天，响应偏慢",
            median_fix_days
        ));
    }
    if open_cve_backlog > 5 {
        score -= 16;
        reasons.push(format!("当前待处理 CVE 积压数量为 {}", open_cve_backlog));
    }
    if has_security_policy && median_fix_days <= 14 && open_cve_backlog <= 2 {
        reasons.push("安全流程较稳定，具备持续修复能力".to_string());
    }
    reasons.extend(missing.iter().map(|key| format!("缺少安全指标: {}", key)));
    finalize_assessment(
        score,
        coverage,
        reasons,
        indicators,
        collect_evidence_refs(&entries),
    )
}

fn build_quality_assessment(raw_evidence: &[Value]) -> EcosystemSubAssessment {
    let entries = entries_by_category(raw_evidence, "quality");
    let indicators = collect_indicators(&entries);
    let required_keys = [
        "dedicated_code_reviewers",
        "required_reviews",
        "signed_releases",
        "provenance_attestation",
        "release_checklist",
    ];
    let (coverage, missing) = coverage_for_keys(&indicators, &required_keys);
    let mut score = 100 - (missing.len() as i32 * 8);
    let mut reasons = vec![format!("质量评估已纳入 {} 个证据条目", entries.len())];

    let dedicated_code_reviewers =
        indicator_i64(&indicators, "dedicated_code_reviewers").unwrap_or(0);
    let required_reviews = indicator_i64(&indicators, "required_reviews").unwrap_or(0);
    let signed_releases = indicator_bool(&indicators, "signed_releases").unwrap_or(false);
    let provenance_attestation =
        indicator_bool(&indicators, "provenance_attestation").unwrap_or(false);
    let release_checklist = indicator_bool(&indicators, "release_checklist").unwrap_or(false);

    if dedicated_code_reviewers == 0 {
        score -= 20;
        reasons.push("未识别到专人代码审查责任人".to_string());
    }
    if required_reviews < 1 {
        score -= 15;
        reasons.push("代码审查门槛偏低".to_string());
    }
    if !signed_releases {
        score -= 24;
        reasons.push("发布物未体现数字签名能力".to_string());
    }
    if !provenance_attestation {
        score -= 12;
        reasons.push("缺少发布来源证明或供应链佐证".to_string());
    }
    if !release_checklist {
        score -= 8;
        reasons.push("发布检查清单流程不明确".to_string());
    }
    if dedicated_code_reviewers >= 2 && signed_releases {
        reasons.push("审查责任人与发布签名机制较明确".to_string());
    }
    reasons.extend(missing.iter().map(|key| format!("缺少质量指标: {}", key)));
    finalize_assessment(
        score,
        coverage,
        reasons,
        indicators,
        collect_evidence_refs(&entries),
    )
}

fn finalize_assessment(
    raw_score: i32,
    coverage: i32,
    mut reasons: Vec<String>,
    indicators: Vec<EcosystemIndicator>,
    evidence_refs: Vec<String>,
) -> EcosystemSubAssessment {
    let score = raw_score.clamp(0, 100);
    reasons.truncate(8);
    EcosystemSubAssessment {
        level: level_from_score(score).to_string(),
        confidence: confidence_from_coverage(coverage).to_string(),
        score,
        coverage,
        reasons,
        evidence_refs,
        indicators,
    }
}

fn build_evidence_catalog(raw_evidence: &[Value]) -> Value {
    let mut category_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut subcategory_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut sources = BTreeSet::new();

    for entry in raw_evidence {
        if let Some(category) = entry.get("assessment_category").and_then(Value::as_str) {
            *category_counts.entry(category.to_string()).or_default() += 1;
        }
        if let Some(subcategory) = entry.get("assessment_subcategory").and_then(Value::as_str) {
            *subcategory_counts
                .entry(subcategory.to_string())
                .or_default() += 1;
        }
        if let Some(source_name) = entry.get("source_name").and_then(Value::as_str) {
            sources.insert(source_name.to_string());
        }
    }

    json!({
        "category_counts": category_counts,
        "subcategory_counts": subcategory_counts,
        "sources": sources.into_iter().collect::<Vec<_>>(),
    })
}

fn entries_by_category<'a>(raw_evidence: &'a [Value], category: &str) -> Vec<&'a Value> {
    raw_evidence
        .iter()
        .filter(|entry| {
            entry
                .get("assessment_category")
                .and_then(Value::as_str)
                .map(|value| value == category)
                .unwrap_or(false)
        })
        .collect()
}

