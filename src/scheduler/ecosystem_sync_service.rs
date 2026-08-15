use anyhow::Result;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::{
    ecosystem::EcosystemService,
    entities::{ecosystem_targets, prelude::*},
};

pub struct EcosystemSyncService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> EcosystemSyncService<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn refresh_active_targets(&self) -> Result<usize> {
        let service = EcosystemService::new(self.db);
        let targets = EcosystemTargets::find()
            .filter(ecosystem_targets::Column::Status.eq("active"))
            .all(self.db)
            .await?;

        let mut refreshed = 0;
        for target in targets {
            service.refresh_target(target.id).await?;
            refreshed += 1;
        }
        Ok(refreshed)
    }
}
