/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. ctscat is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use chrono::Utc;
use regex::Regex;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::entities::{
    backport_candidates, backport_candidates::Entity as BackportCandidates, distros, l0_commits,
    packages, tracking,
};
use crate::telemetry::Telemetry;

/// Backport 候选生成摘要
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BackportSummary {
    pub candidates_created: usize,
    pub candidates_skipped: usize,
}

/// 回合建议生成器
pub struct BackportAdvisor<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> BackportAdvisor<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// 针对指定软件包生成回合候选
    pub async fn generate_for_package(&self, package_id: i32) -> Result<BackportSummary> {
        let package = packages::Entity::find_by_id(package_id)
            .one(self.db)
            .await?
            .ok_or_else(|| anyhow!("package {} not found", package_id))?;

        let trackings = tracking::Entity::find()
            .filter(tracking::Column::PackageId.eq(package_id))
            .all(self.db)
            .await?;

        if trackings.is_empty() {
            return Ok(BackportSummary::default());
        }

        let distro_ids: HashSet<i32> = trackings.iter().map(|t| t.distro_id).collect();
        let distros = distros::Entity::find()
            .filter(distros::Column::Id.is_in(distro_ids.clone()))
            .all(self.db)
            .await?;

        let mut distro_map: HashMap<i32, String> = HashMap::new();
        for distro in distros {
            distro_map.insert(distro.id, format!("{} {}", distro.name, distro.version));
        }

        let commits = l0_commits::Entity::find()
            .filter(l0_commits::Column::PackageId.eq(package_id))
            .all(self.db)
            .await?;

        let mut summary = BackportSummary::default();

        for commit in commits {
            let version = extract_version(&commit.summary).unwrap_or_else(|| "unknown".to_string());

            for track in &trackings {
                let exists = BackportCandidates::find()
                    .filter(backport_candidates::Column::PackageId.eq(package_id))
