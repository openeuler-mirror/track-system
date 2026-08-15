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
            ) {
                summary.skipped_not_due += 1;
                continue;
            }

            summary.due_packages += 1;
            match maintenance_service.refresh_package(package.id).await {
                Ok(_) => summary.refreshed_packages += 1,
                Err(_) => summary.failed_packages += 1,
            }
        }

        Ok(summary)
    }
}

fn should_refresh_package(
    now: DateTime<Utc>,
    sync_interval_hours: i32,
    latest_report_at: Option<DateTime<Utc>>,
) -> bool {
    let interval_hours = sync_interval_hours.max(1) as i64;
    match latest_report_at {
        Some(last) => (now - last).num_hours() >= interval_hours,
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn should_refresh_package_when_no_previous_report() {
        assert!(should_refresh_package(Utc::now(), 24, None));
    }

    #[test]
    fn should_refresh_package_when_interval_elapsed() {
        let now = Utc::now();
        let last = now - Duration::hours(25);
        assert!(should_refresh_package(now, 24, Some(last)));
    }

    #[test]
    fn should_not_refresh_package_before_interval_elapsed() {
        let now = Utc::now();
        let last = now - Duration::hours(23);
        assert!(!should_refresh_package(now, 24, Some(last)));
    }
}
