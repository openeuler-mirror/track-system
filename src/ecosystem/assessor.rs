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

fn collect_indicators(entries: &[&Value]) -> Vec<EcosystemIndicator> {
    let mut indicators = Vec::new();
    for entry in entries {
        let source_name = entry
            .get("source_name")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let subcategory = entry
            .get("assessment_subcategory")
            .and_then(Value::as_str)
            .unwrap_or("general");

        if let Some(data) = entry.get("data").and_then(Value::as_object) {
            for (key, value) in data {
                indicators.push(EcosystemIndicator {
                    key: key.to_string(),
                    label: indicator_label(key).to_string(),
                    value: value.clone(),
                    status: indicator_status(value).to_string(),
                    source: format!("{}:{}", source_name, subcategory),
                });
            }
        }
    }
    indicators
}

fn collect_evidence_refs(entries: &[&Value]) -> Vec<String> {
    let mut refs = BTreeSet::new();
    for entry in entries {
        let source_name = entry
            .get("source_name")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let subcategory = entry
            .get("assessment_subcategory")
            .and_then(Value::as_str)
            .unwrap_or("general");
        refs.insert(format!("{}:{}", source_name, subcategory));
    }
    refs.into_iter().collect()
}

fn coverage_for_keys(
    indicators: &[EcosystemIndicator],
    required_keys: &[&str],
) -> (i32, Vec<String>) {
    let mut missing = Vec::new();
    let mut present = 0;
    for key in required_keys {
        if indicators
            .iter()
            .any(|indicator| indicator.key == *key && indicator.status != "missing")
        {
            present += 1;
        } else {
            missing.push((*key).to_string());
        }
    }
    let coverage = if required_keys.is_empty() {
        100
    } else {
        (present * 100 / required_keys.len()) as i32
    };
    (coverage, missing)
}

fn contains_risk_phrase(entries: &[&Value], keys: &[&str]) -> bool {
    entries.iter().any(|entry| {
        entry
            .get("data")
            .and_then(Value::as_object)
            .map(|data| {
                keys.iter().any(|key| {
                    data.get(*key)
                        .and_then(Value::as_str)
                        .map(|value| {
                            value.contains("限制")
                                || value.contains("制裁")
                                || value.contains("下架")
                                || value.contains("风控")
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    })
}

fn indicator_i64(indicators: &[EcosystemIndicator], key: &str) -> Option<i64> {
    indicators
        .iter()
        .find(|indicator| indicator.key == key)
        .and_then(|indicator| match &indicator.value {
            Value::Number(number) => number.as_i64(),
            Value::String(value) => value.parse::<i64>().ok(),
            _ => None,
        })
}

fn indicator_bool(indicators: &[EcosystemIndicator], key: &str) -> Option<bool> {
    indicators
        .iter()
        .find(|indicator| indicator.key == key)
        .and_then(|indicator| match &indicator.value {
            Value::Bool(value) => Some(*value),
            Value::String(value) => match value.to_ascii_lowercase().as_str() {
                "true" | "yes" | "1" => Some(true),
                "false" | "no" | "0" => Some(false),
                _ => None,
            },
            _ => None,
        })
}

fn indicator_datetime(indicators: &[EcosystemIndicator], key: &str) -> Option<DateTime<Utc>> {
    indicators
        .iter()
        .find(|indicator| indicator.key == key)
        .and_then(|indicator| indicator.value.as_str())
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
}

fn indicator_status(value: &Value) -> &'static str {
    match value {
        Value::Null => "missing",
        Value::Bool(true) => "present",
        Value::Bool(false) => "absent",
        Value::String(text) if text.trim().is_empty() => "missing",
        Value::Array(items) if items.is_empty() => "missing",
        Value::Object(items) if items.is_empty() => "missing",
        _ => "present",
    }
}

fn indicator_label(key: &str) -> &str {
    match key {
        "organization_structure" => "组织架构",
        "foundation_status" => "基金会情况",
        "version_lifecycle" => "版本生命周期",
        "license_policy" => "许可证情况",
        "cla_policy" => "CLA 协议",
        "basic_info" => "基础信息",
        "trade_controls" => "贸易管制情况",
        "ip_policy" => "知识产权情况",
        "government_takedown_policy" => "政府下架情况",
        "top_contributors" => "主要贡献者及贡献次数",
        "foundation_list" => "基金会列表",
        "donor_countries" => "捐献者所属国家",
        "commit_total" => "Commit 总数",
        "commits_last_12_months" => "近 12 月 Commit 数",
        "committers_last_12_months" => "近 12 月 Committer 数",
        "last_commit_at" => "最近一次 Commit 时间",
        "stars" => "标星数",
        "forks" => "Fork 数",
        "has_security_policy" => "安全策略",
        "cve_fix_commits_last_12_months" => "近 12 月 CVE 修复 Commit 数",
        "cve_linked_issues_last_12_months" => "近 12 月 CVE 关联 Issue 数",
        "median_cve_fix_days" => "CVE 修复中位时长",
        "open_cve_backlog" => "待处理 CVE 积压",
        "dedicated_code_reviewers" => "专职审查人员数",
        "required_reviews" => "合并前必需审查数",
        "signed_releases" => "发布物数字签名",
        "provenance_attestation" => "发布来源证明",
        "release_checklist" => "发布检查清单",
        _ => key,
    }
}

fn level_from_score(score: i32) -> &'static str {
    if score >= 80 {
        "LOW"
    } else if score >= 60 {
        "MEDIUM"
    } else {
        "HIGH"
    }
}

fn confidence_from_coverage(coverage: i32) -> &'static str {
    if coverage >= 85 {
        "HIGH"
    } else if coverage >= 60 {
        "MEDIUM"
    } else {
        "LOW"
    }
}

fn worst_level<'a>(levels: impl IntoIterator<Item = &'a str>) -> String {
    levels
        .into_iter()
        .max_by_key(|level| level_weight(level))
        .unwrap_or("UNKNOWN")
        .to_string()
}

fn aggregate_confidence<'a>(levels: impl IntoIterator<Item = &'a str>) -> String {
    let min_level = levels
        .into_iter()
        .min_by_key(|level| confidence_weight(level))
        .unwrap_or("LOW");
    min_level.to_string()
}

fn level_weight(level: &str) -> i32 {
    match level {
        "HIGH" => 3,
        "MEDIUM" => 2,
        "LOW" => 1,
        _ => 0,
    }
}

fn confidence_weight(level: &str) -> i32 {
    match level {
        "LOW" => 1,
        "MEDIUM" => 2,
        "HIGH" => 3,
        _ => 0,
    }
}

fn freshness_level(refresh_interval_hours: i32) -> &'static str {
    if refresh_interval_hours <= 24 {
        "LOW"
    } else if refresh_interval_hours <= 72 {
        "MEDIUM"
    } else {
        "HIGH"
    }
}

fn freshness_score(refresh_interval_hours: i32) -> i32 {
    match freshness_level(refresh_interval_hours) {
        "LOW" => 90,
        "MEDIUM" => 70,
        _ => 50,
    }
}

