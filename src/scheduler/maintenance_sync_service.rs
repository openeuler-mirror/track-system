use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, EntityTrait};

use crate::{ecosystem::maintenance::MaintenanceService, entities::prelude::*};

#[derive(Debug, Default, Clone)]
pub struct MaintenanceSyncSummary {
    pub scanned_packages: usize,
    pub due_packages: usize,
    pub refreshed_packages: usize,
    pub failed_packages: usize,
    pub skipped_no_repo: usize,
    pub skipped_not_due: usize,
}

pub struct MaintenanceSyncService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> MaintenanceSyncService<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn refresh_due_packages(&self) -> Result<MaintenanceSyncSummary> {
        let maintenance_service = MaintenanceService::new(self.db);
        let packages = Packages::find().all(self.db).await?;
        let now = Utc::now();

        let mut summary = MaintenanceSyncSummary {
            scanned_packages: packages.len(),
            ..Default::default()
        };

        for package in packages {
            if package
                .l0_repo_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                summary.skipped_no_repo += 1;
                continue;
            }

            let latest = maintenance_service.latest_report(package.id).await?;
            if !should_refresh_package(
                now,
                package.sync_interval_hours,
                latest.as_ref().map(|report| report.generated_at),
