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

