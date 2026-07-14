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
