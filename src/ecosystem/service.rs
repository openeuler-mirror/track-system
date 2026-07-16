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

        if platform.contains("gitee")
            || platform.contains("openeuler")
            || target.target_type.contains("community")
        {
            evidence.extend(GiteeEcosystemCollector::new().collect(target).await?);
        }
        if platform.contains("github") {
            evidence.extend(GitHubEcosystemCollector::new().collect(target).await?);
        }
        if platform.contains("atomgit") {
            evidence.extend(AtomGitEcosystemCollector::new().collect(target).await?);
        }

        Ok(evidence)
    }
