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
