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
