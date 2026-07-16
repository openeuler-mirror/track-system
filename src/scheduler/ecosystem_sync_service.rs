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
