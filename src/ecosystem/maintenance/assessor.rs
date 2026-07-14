use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::entities::packages;

use super::report::MaintenanceAssessment;
use super::types::{
    MaintenanceDimension, MaintenanceIndicator, MaintenanceRefreshContext, MaintenanceSubAssessment,
};

const ASSESSMENT_VERSION: &str = "maintenance-assessment-v1";

pub fn assess_target(
    package: &packages::Model,
    evidence_summary: Value,
    raw_evidence: &[Value],
) -> MaintenanceAssessment {
    let refreshed_at = Utc::now();
    let context = MaintenanceRefreshContext {
        package_id: package.id,
        package_name: package.name.clone(),
        l0_repo_url: package.l0_repo_url.clone(),
        refreshed_at,
