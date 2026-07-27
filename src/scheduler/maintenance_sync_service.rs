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
