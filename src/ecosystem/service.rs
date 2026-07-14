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

