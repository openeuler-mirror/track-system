use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::entities::{maintenance_evidence_snapshots, maintenance_reports, packages, prelude::*};

use super::assessor::assess_target;
use super::collectors::{
    AtomGitMaintenanceCollector, GenericGitMaintenanceCollector, GitHubMaintenanceCollector,
    GitLabMaintenanceCollector, GiteeMaintenanceCollector, PagureMaintenanceCollector,
};
use super::report::{MaintenanceAssessment, MaintenanceRefreshResult};
use tracing::warn;

pub struct MaintenanceService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> MaintenanceService<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn refresh_package(&self, package_id: i32) -> Result<MaintenanceRefreshResult> {
        let package = Packages::find_by_id(package_id)
            .one(self.db)
            .await
            .context("query package failed")?
            .ok_or_else(|| anyhow!("package {} not found", package_id))?;

        let now = Utc::now();
        let evidence_payloads = self.collect_evidence(&package).await?;

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
            let http_status = payload
                .get("http_status")
                .and_then(Value::as_i64)
                .map(|value| value as i32)
                .or(Some(200));

            let evidence = maintenance_evidence_snapshots::ActiveModel {
                package_id: Set(package.id),
                source_type: Set(source_type),
                source_name: Set(source_name),
                source_url: Set(source_url),
                http_status: Set(http_status),
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

        let evidence_summary = self.build_evidence_summary(&package, &evidence_payloads);
        let assessment = assess_target(&package, evidence_summary.clone(), &evidence_payloads);
        let report = self.save_report(package.id, assessment).await?;

        Ok(MaintenanceRefreshResult {
            package_id,
            evidence_count: evidence_payloads.len(),
            report_id: report.id,
            generated_at: report.generated_at,
        })
    }

    pub async fn latest_report(
        &self,
        package_id: i32,
    ) -> Result<Option<maintenance_reports::Model>> {
        let report = MaintenanceReports::find()
            .filter(maintenance_reports::Column::PackageId.eq(package_id))
            .order_by_desc(maintenance_reports::Column::GeneratedAt)
            .one(self.db)
            .await?;
        Ok(report)
    }

    async fn collect_evidence(&self, package: &packages::Model) -> Result<Vec<Value>> {
        let mut evidence = vec![json!({
            "source_type": "package_definition",
            "source_name": "package",
            "source_url": package.l0_repo_url.clone().unwrap_or_default(),
            "http_status": 200,
            "assessment_category": "maintenance",
            "assessment_subcategory": "package_definition",
            "package_id": package.id,
            "data": {
                "basic_info": package.name,
                "package_name": package.name,
                "package_level": package.level,
                "l0_repo_url": package.l0_repo_url,
                "description": package.description,
            }
        })];

        evidence.extend(self.collect_platform_evidence(package).await?);

        Ok(evidence)
    }

    async fn collect_platform_evidence(&self, package: &packages::Model) -> Result<Vec<Value>> {
        let mut evidence = Vec::new();

        if GitHubMaintenanceCollector::matches_package(package) {
            evidence.extend(GitHubMaintenanceCollector::new().collect(package).await?);
        } else if GitLabMaintenanceCollector::matches_package(package) {
            evidence.extend(GitLabMaintenanceCollector::new().collect(package).await?);
        } else if GiteeMaintenanceCollector::matches_package(package) {
            evidence.extend(GiteeMaintenanceCollector::new().collect(package).await?);
        } else if AtomGitMaintenanceCollector::matches_package(package) {
            match AtomGitMaintenanceCollector::new().collect(package).await {
                Ok(mut specialized) => evidence.append(&mut specialized),
                Err(error) => warn!(
                    package = package.name,
                    error = %error,
                    "AtomGit 平台维护指标采集失败，跳过平台证据补充"
                ),
            }
        } else if PagureMaintenanceCollector::matches_package(package) {
            evidence.extend(PagureMaintenanceCollector::new().collect(package).await?);
        } else if GenericGitMaintenanceCollector::matches_package(package) {
            evidence.extend(
                GenericGitMaintenanceCollector::new()
                    .collect(package)
                    .await?,
            );
        }

        if GenericGitMaintenanceCollector::matches_package(package) {
            match GenericGitMaintenanceCollector::new()
                .collect_version_catalog(package)
                .await
            {
                Ok(version_catalog) => evidence.push(version_catalog),
                Err(error) => warn!(
                    package = package.name,
                    repo_url = package.l0_repo_url.as_deref().unwrap_or_default(),
                    error = %error,
                    "L0 Git tag 版本目录采集失败，跳过版本目录证据"
                ),
            }
        }

        Ok(evidence)
    }

    fn build_evidence_summary(
        &self,
        package: &packages::Model,
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
            "package_id": package.id,
            "package_name": package.name,
            "package_level": package.level,
            "l0_repo_url": package.l0_repo_url,
            "category_counts": category_counts,
            "subcategory_counts": subcategory_counts,
            "sources": source_names.into_iter().collect::<Vec<_>>(),
        })
    }

    async fn save_report(
        &self,
        package_id: i32,
        assessment: MaintenanceAssessment,
    ) -> Result<maintenance_reports::Model> {
        let now = Utc::now();
        let report = maintenance_reports::ActiveModel {
            package_id: Set(package_id),
            report_type: Set(assessment.report_type),
            status: Set("completed".to_string()),
            overall_risk: Set(assessment.overall_risk),
            confidence: Set(assessment.confidence),
            summary: Set(assessment.summary),
            dimensions: Set(json!(assessment.dimensions)),
            evidence_summary: Set(Some(assessment.evidence_summary)),
            report_payload: Set(assessment.report_payload),
            generated_at: Set(assessment.generated_at),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        report
            .insert(self.db)
            .await
            .context("insert maintenance report failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecosystem::maintenance::types::{MaintenanceDimension, MaintenanceSubAssessment};
    use sea_orm::{DatabaseBackend, MockDatabase};

    fn package(repo_url: Option<&str>) -> packages::Model {
        let now = Utc::now();
        packages::Model {
            id: 5,
            name: "openssl".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: repo_url.map(str::to_string),
            description: Some("crypto library".to_string()),
            created_at: now,
            updated_at: now,
        }
    }

    fn assessment() -> MaintenanceAssessment {
        let mut dimensions = BTreeMap::new();
        dimensions.insert(
            "activity_risk".to_string(),
            MaintenanceDimension {
                level: "medium".to_string(),
                score: 70,
                reasons: vec!["moderate activity".to_string()],
            },
        );

        MaintenanceAssessment {
            report_type: "maintenance_profile".to_string(),
            overall_risk: "medium".to_string(),
            confidence: "high".to_string(),
            summary: "maintenance summary".to_string(),
            section: MaintenanceSubAssessment {
                level: "medium".to_string(),
                confidence: "high".to_string(),
                score: 70,
                coverage: 80,
                reasons: vec!["moderate activity".to_string()],
                evidence_refs: vec!["package".to_string()],
                indicators: Vec::new(),
            },
            dimensions,
            evidence_summary: json!({"evidence_count": 1}),
            report_payload: json!({"context": {"package_id": 5}}),
            generated_at: Utc::now(),
        }
    }

    #[test]
    fn evidence_summary_counts_categories_subcategories_and_sources() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let service = MaintenanceService::new(&db);
        let package = package(Some("https://github.com/openssl/openssl.git"));
        let evidence = vec![
            json!({
                "assessment_category": "maintenance",
                "assessment_subcategory": "package_definition",
                "source_name": "package"
            }),
