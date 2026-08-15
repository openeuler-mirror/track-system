use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::collectors::{
    atomgit::ecosystem::AtomGitEcosystemCollector, gitee::ecosystem::GiteeEcosystemCollector,
    github::ecosystem::GitHubEcosystemCollector,
};
use crate::ecosystem::assessor::assess_target;
use crate::ecosystem::report::{EcosystemAssessment, EcosystemRefreshResult};
use crate::ecosystem::sbom_sync::SbomCommunitySyncClient;
use crate::ecosystem::targets::{
    AtomGitPlatformCollector, GitHubPlatformCollector, OpenEulerCommunityCollector,
};
use crate::entities::{
    ecosystem_evidence_snapshots, ecosystem_reports, ecosystem_targets, prelude::*,
};

pub struct EcosystemService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> EcosystemService<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn refresh_target(&self, target_id: i32) -> Result<EcosystemRefreshResult> {
        let target = EcosystemTargets::find_by_id(target_id)
            .one(self.db)
            .await
            .context("query ecosystem target failed")?
            .ok_or_else(|| anyhow!("ecosystem target {} not found", target_id))?;

        let now = Utc::now();
        let evidence_payloads = self.collect_evidence(&target).await?;

        for payload in &evidence_payloads {
            let source_type = payload
                .get("source_type")
                .and_then(Value::as_str)
                .unwrap_or("placeholder")
                .to_string();
            let source_name = payload
                .get("source_name")
                .and_then(Value::as_str)
                .unwrap_or("placeholder")
                .to_string();
            let source_url = payload
                .get("source_url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();

            let evidence = ecosystem_evidence_snapshots::ActiveModel {
                target_id: Set(target.id),
                source_type: Set(source_type),
                source_name: Set(source_name),
                source_url: Set(source_url),
                http_status: Set(Some(200)),
                content_hash: Set(None),
                raw_payload: Set(payload.clone()),
                normalized_signals: Set(payload.get("data").cloned()),
                collected_at: Set(now),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            };
            evidence.insert(self.db).await?;
        }

        let evidence_summary = self.build_evidence_summary(&target, &evidence_payloads);
        let assessment = assess_target(&target, evidence_summary.clone(), &evidence_payloads);
        let report = self.save_report(target.id, assessment).await?;
        self.sync_report_to_sbom(&target, &report).await;

        let mut target_model: ecosystem_targets::ActiveModel = target.into();
        target_model.last_collected_at = Set(Some(now));
        target_model.last_report_at = Set(Some(now));
        target_model.last_error = Set(None);
        target_model.updated_at = Set(now);
        target_model.update(self.db).await?;

        Ok(EcosystemRefreshResult {
            target_id,
            evidence_count: evidence_payloads.len(),
            report_id: report.id,
            generated_at: report.generated_at,
        })
    }

    async fn sync_report_to_sbom(
        &self,
        target: &ecosystem_targets::Model,
        report: &ecosystem_reports::Model,
    ) {
        let client = match SbomCommunitySyncClient::from_env() {
            Ok(Some(client)) => client,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(
                    target_id = target.id,
                    report_id = report.id,
                    error = %error,
                    "SBOM 社区同步配置无效，跳过同步"
                );
                return;
            }
        };

        match client.sync_report(target, report).await {
            Ok(response) => {
                tracing::info!(
                    target_id = target.id,
                    report_id = report.id,
                    sbom_code = response.code,
                    sbom_msg = %response.msg,
                    "SBOM 社区同步成功"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target_id = target.id,
                    report_id = report.id,
                    error = %error,
                    "SBOM 社区同步失败，生态评估报告已保留"
                );
            }
        }
    }

    pub async fn latest_report(&self, target_id: i32) -> Result<Option<ecosystem_reports::Model>> {
        let report = EcosystemReports::find()
            .filter(ecosystem_reports::Column::TargetId.eq(target_id))
            .order_by_desc(ecosystem_reports::Column::GeneratedAt)
            .one(self.db)
            .await?;
        Ok(report)
    }

    async fn collect_evidence(&self, target: &ecosystem_targets::Model) -> Result<Vec<Value>> {
        let mut evidence = vec![
            json!({
                "source_type": "target_definition",
                "source_name": "ecosystem_target",
                "source_url": target.homepage_url.clone().unwrap_or_default(),
                "assessment_category": "source",
                "assessment_subcategory": "target_definition",
                "target_id": target.id,
                "data": {
                    "basic_info": target.name,
                    "target_type": target.target_type,
                    "platform": target.platform,
                    "rule_profile": target.rule_profile,
                }
            }),
            json!({
                "source_type": "rule_profile",
                "source_name": "assessment_profile",
                "source_url": target.api_base_url.clone().unwrap_or_default(),
                "assessment_category": "quality",
                "assessment_subcategory": "assessment_profile",
                "data": {
                    "release_checklist": true,
                    "required_reviews": 1,
                    "refresh_interval_hours": target.refresh_interval_hours,
                    "status": target.status,
                }
            }),
        ];

        evidence.extend(self.collect_metadata_evidence(target));
        evidence.extend(self.collect_platform_evidence(target).await?);

        Ok(evidence)
    }

    fn collect_metadata_evidence(&self, target: &ecosystem_targets::Model) -> Vec<Value> {
        let mut evidence = Vec::new();
        let Some(metadata) = target.metadata.as_ref() else {
            return evidence;
        };

        if let Some(source_assessment) =
            metadata.get("source_assessment").and_then(Value::as_object)
        {
            for (subcategory, data) in source_assessment {
                evidence.push(self.build_metadata_record(
                    target,
                    "metadata_source_assessment",
                    "metadata_source",
                    "source",
                    subcategory,
                    data.clone(),
                ));
            }
        }

        if let Some(data) = metadata.get("maintenance_assessment") {
            evidence.push(self.build_metadata_record(
                target,
                "metadata_maintenance_assessment",
                "metadata_maintenance",
                "maintenance",
                "repository_activity",
                data.clone(),
            ));
        }

        if let Some(data) = metadata.get("security_assessment") {
            evidence.push(self.build_metadata_record(
                target,
                "metadata_security_assessment",
                "metadata_security",
                "security",
                "cve_process",
                data.clone(),
            ));
        }

        if let Some(data) = metadata.get("quality_assessment") {
            evidence.push(self.build_metadata_record(
                target,
                "metadata_quality_assessment",
                "metadata_quality",
                "quality",
                "release_quality",
                data.clone(),
            ));
        }

        evidence
    }

    async fn collect_platform_evidence(
        &self,
        target: &ecosystem_targets::Model,
    ) -> Result<Vec<Value>> {
        let platform = target
            .platform
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();

        let mut evidence = Vec::new();

        if OpenEulerCommunityCollector::matches_target(target) {
            evidence.extend(OpenEulerCommunityCollector::new().collect(target).await?);
        }
        if GitHubPlatformCollector::matches_target(target) {
            evidence.extend(GitHubPlatformCollector::new().collect(target).await?);
        }
        if AtomGitPlatformCollector::matches_target(target) {
            evidence.extend(AtomGitPlatformCollector::new().collect(target).await?);
        }
        if platform.contains("gitee") {
            evidence.extend(GiteeEcosystemCollector::new().collect(target).await?);
        }
        if platform.contains("github") && !GitHubPlatformCollector::matches_target(target) {
            evidence.extend(GitHubEcosystemCollector::new().collect(target).await?);
        }
        if platform.contains("atomgit") && !AtomGitPlatformCollector::matches_target(target) {
            evidence.extend(AtomGitEcosystemCollector::new().collect(target).await?);
        }

        Ok(evidence)
    }

    fn build_metadata_record(
        &self,
        target: &ecosystem_targets::Model,
        source_type: &str,
        source_name: &str,
        assessment_category: &str,
        assessment_subcategory: &str,
        data: Value,
    ) -> Value {
        json!({
            "source_type": source_type,
            "source_name": source_name,
            "source_url": target.homepage_url.clone().unwrap_or_default(),
            "assessment_category": assessment_category,
            "assessment_subcategory": assessment_subcategory,
            "data": data,
        })
    }

    fn build_evidence_summary(
        &self,
        target: &ecosystem_targets::Model,
        evidence_payloads: &[Value],
    ) -> Value {
        let mut category_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut subcategory_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut source_names = BTreeSet::new();

        for payload in evidence_payloads {
            if let Some(category) = payload.get("assessment_category").and_then(Value::as_str) {
                *category_counts.entry(category.to_string()).or_default() += 1;
            }
            if let Some(subcategory) = payload
                .get("assessment_subcategory")
                .and_then(Value::as_str)
            {
                *subcategory_counts
                    .entry(subcategory.to_string())
                    .or_default() += 1;
            }
            if let Some(source_name) = payload.get("source_name").and_then(Value::as_str) {
                source_names.insert(source_name.to_string());
            }
        }

        json!({
            "evidence_count": evidence_payloads.len(),
            "target_type": target.target_type,
            "platform": target.platform,
            "rule_profile": target.rule_profile,
            "category_counts": category_counts,
            "subcategory_counts": subcategory_counts,
            "sources": source_names.into_iter().collect::<Vec<_>>(),
        })
    }

    async fn save_report(
        &self,
        target_id: i32,
        assessment: EcosystemAssessment,
    ) -> Result<ecosystem_reports::Model> {
        let now = Utc::now();
        let report = ecosystem_reports::ActiveModel {
            target_id: Set(target_id),
            report_type: Set(assessment.report_type),
            status: Set("completed".to_string()),
            overall_risk: Set(assessment.overall_risk),
            confidence: Set(assessment.confidence),
            summary: Set(assessment.summary),
            dimensions: Set(serde_json::to_value(assessment.dimensions)?),
            evidence_summary: Set(Some(assessment.evidence_summary)),
            report_payload: Set(assessment.report_payload),
            generated_at: Set(assessment.generated_at),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };
        Ok(report.insert(self.db).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecosystem::types::{
        EcosystemAssessmentSections, EcosystemDimension, EcosystemSubAssessment,
    };
    use sea_orm::{DatabaseBackend, MockDatabase};
    use serial_test::serial;
    use std::ffi::OsString;

    struct EnvGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvGuard {
        fn remove(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn target_with_metadata(metadata: Option<Value>) -> ecosystem_targets::Model {
        let now = Utc::now();
        ecosystem_targets::Model {
            id: 7,
            name: "openEuler".to_string(),
            target_type: "community".to_string(),
            platform: Some("openeuler".to_string()),
            role: "upstream".to_string(),
            homepage_url: Some("https://www.openeuler.org".to_string()),
            api_base_url: Some("https://api.openeuler.org".to_string()),
            owner: Some("openeuler".to_string()),
            repo: Some("community".to_string()),
            default_branch: Some("master".to_string()),
            status: "active".to_string(),
            refresh_interval_hours: 24,
            rule_profile: "openeuler_community".to_string(),
            metadata,
            last_collected_at: None,
            last_report_at: None,
            last_error: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn sub_assessment(level: &str, score: i32) -> EcosystemSubAssessment {
        EcosystemSubAssessment {
            level: level.to_string(),
            confidence: "high".to_string(),
            score,
            coverage: 90,
            reasons: vec![format!("{level} reason")],
            evidence_refs: vec!["metadata_source".to_string()],
            indicators: Vec::new(),
        }
    }

    fn assessment() -> EcosystemAssessment {
        let sections = EcosystemAssessmentSections {
            source: sub_assessment("low", 90),
            maintenance: sub_assessment("medium", 70),
            security: sub_assessment("high", 50),
            quality: sub_assessment("low", 85),
        };
        let mut dimensions = BTreeMap::new();
        dimensions.insert(
            "source_risk".to_string(),
            EcosystemDimension {
                level: "low".to_string(),
                score: 90,
                reasons: vec!["source ok".to_string()],
            },
        );

        EcosystemAssessment {
            report_type: "ecosystem_profile".to_string(),
            overall_risk: "high".to_string(),
            confidence: "high".to_string(),
            summary: "summary".to_string(),
            sections,
            dimensions,
            evidence_summary: json!({"evidence_count": 2}),
            report_payload: json!({"context": {"target_id": 7}}),
            generated_at: Utc::now(),
        }
    }

    #[test]
    fn metadata_evidence_expands_all_supported_sections() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let service = EcosystemService::new(&db);
        let target = target_with_metadata(Some(json!({
            "source_assessment": {
                "organization_structure": {"foundation_status": "foundation"},
                "license_policy": {"license_policy": "MulanPSL-2.0"}
            },
            "maintenance_assessment": {"commits_last_12_months": 120},
            "security_assessment": {"has_security_policy": true},
            "quality_assessment": {"signed_releases": true}
        })));

        let evidence = service.collect_metadata_evidence(&target);

        assert_eq!(evidence.len(), 5);
        assert!(evidence
            .iter()
            .any(|item| item["assessment_subcategory"] == "organization_structure"));
        assert!(evidence
            .iter()
            .any(|item| item["assessment_category"] == "security"));
        assert!(evidence
            .iter()
            .any(|item| item["source_url"] == "https://www.openeuler.org"));
    }

    #[test]
    fn metadata_evidence_is_empty_without_metadata() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let service = EcosystemService::new(&db);
        let target = target_with_metadata(None);

        assert!(service.collect_metadata_evidence(&target).is_empty());
    }

    #[test]
    fn evidence_summary_counts_categories_subcategories_and_sources() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let service = EcosystemService::new(&db);
        let target = target_with_metadata(None);
        let evidence = vec![
            json!({
                "assessment_category": "source",
                "assessment_subcategory": "license_policy",
                "source_name": "metadata_source"
            }),
            json!({
                "assessment_category": "source",
                "assessment_subcategory": "license_policy",
                "source_name": "metadata_source"
            }),
            json!({
                "assessment_category": "security",
                "assessment_subcategory": "cve_process",
                "source_name": "metadata_security"
            }),
        ];

        let summary = service.build_evidence_summary(&target, &evidence);

        assert_eq!(summary["evidence_count"], 3);
        assert_eq!(summary["target_type"], "community");
        assert_eq!(summary["category_counts"]["source"], 2);
        assert_eq!(summary["subcategory_counts"]["license_policy"], 2);
        assert_eq!(
            summary["sources"],
            json!(["metadata_security", "metadata_source"])
        );
    }

    #[tokio::test]
    async fn latest_report_queries_latest_target_report() {
        let now = Utc::now();
        let report = ecosystem_reports::Model {
            id: 9,
            target_id: 7,
            report_type: "ecosystem_profile".to_string(),
            status: "completed".to_string(),
            overall_risk: "medium".to_string(),
            confidence: "high".to_string(),
            summary: "latest".to_string(),
            dimensions: json!({}),
            evidence_summary: Some(json!({"evidence_count": 2})),
            report_payload: json!({}),
            generated_at: now,
            created_at: now,
            updated_at: now,
        };
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results(vec![vec![report.clone()]])
            .into_connection();
        let service = EcosystemService::new(&db);

        let latest = service.latest_report(7).await.unwrap();

        assert_eq!(latest, Some(report));
    }

    #[tokio::test]
    async fn save_report_maps_assessment_to_report_model() {
        let now = Utc::now();
        let report = ecosystem_reports::Model {
            id: 11,
            target_id: 7,
            report_type: "ecosystem_profile".to_string(),
            status: "completed".to_string(),
            overall_risk: "high".to_string(),
            confidence: "high".to_string(),
            summary: "summary".to_string(),
            dimensions: json!({"source_risk": {"level": "low"}}),
            evidence_summary: Some(json!({"evidence_count": 2})),
            report_payload: json!({"context": {"target_id": 7}}),
            generated_at: now,
            created_at: now,
            updated_at: now,
        };
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_exec_results([sea_orm::MockExecResult {
                last_insert_id: 11,
                rows_affected: 1,
            }])
            .append_query_results(vec![vec![report.clone()]])
            .into_connection();
        let service = EcosystemService::new(&db);

        let saved = service.save_report(7, assessment()).await.unwrap();

        assert_eq!(saved.id, 11);
        assert_eq!(saved.target_id, 7);
        assert_eq!(saved.overall_risk, "high");
        assert_eq!(saved.status, "completed");
    }

    #[tokio::test]
    #[serial]
    async fn sbom_sync_is_noop_when_disabled() {
        let _enabled = EnvGuard::remove("SBOM_COMMUNITY_SYNC_ENABLED");
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let service = EcosystemService::new(&db);
        let target = target_with_metadata(None);
        let now = Utc::now();
        let report = ecosystem_reports::Model {
            id: 12,
            target_id: target.id,
            report_type: "ecosystem_profile".to_string(),
            status: "completed".to_string(),
            overall_risk: "LOW".to_string(),
            confidence: "HIGH".to_string(),
            summary: "summary".to_string(),
            dimensions: json!({}),
            evidence_summary: Some(json!({"evidence_count": 2})),
            report_payload: json!({}),
            generated_at: now,
            created_at: now,
            updated_at: now,
        };

        service.sync_report_to_sbom(&target, &report).await;
    }
}
