/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

use anyhow::{anyhow, Context, Result};
use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::{
    ecosystem::maintenance::collectors::generic_git::warm_cached_mirror,
    entities::{packages, prelude::Packages},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct L0RepoCacheWarmItem {
    pub package_id: i32,
    pub package_name: String,
    pub repo_url: Option<String>,
    pub cache_path: Option<String>,
    pub default_branch: Option<String>,
    pub cache_retained: bool,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct L0RepoCacheWarmSummary {
    pub scanned_packages: usize,
    pub warmed_packages: usize,
    pub skipped_no_repo: usize,
    pub failed_packages: usize,
    pub results: Vec<L0RepoCacheWarmItem>,
}

pub struct L0RepoCacheService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> L0RepoCacheService<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn warm_package(&self, package_id: i32) -> Result<L0RepoCacheWarmItem> {
        let package = Packages::find_by_id(package_id)
            .one(self.db)
            .await
            .context("query package failed")?
            .ok_or_else(|| anyhow!("package {} not found", package_id))?;

        self.warm_package_model(package).await
    }

    pub async fn warm_all_packages(&self) -> Result<L0RepoCacheWarmSummary> {
        let packages = Packages::find()
            .order_by_asc(packages::Column::Id)
            .all(self.db)
            .await
            .context("query packages failed")?;

        let mut summary = L0RepoCacheWarmSummary {
            scanned_packages: packages.len(),
            ..Default::default()
        };

        for package in packages {
            let package_id = package.id;
            let package_name = package.name.clone();
            let repo_url = package.l0_repo_url.clone();
            match self.warm_package_model(package).await {
                Ok(item) => {
                    match item.status.as_str() {
                        "warmed" => summary.warmed_packages += 1,
                        "skipped" => summary.skipped_no_repo += 1,
                        "failed" => summary.failed_packages += 1,
                        _ => {}
                    }
                    summary.results.push(item);
                }
                Err(error) => {
                    summary.failed_packages += 1;
                    summary.results.push(L0RepoCacheWarmItem {
                        package_id,
                        package_name,
                        repo_url,
                        cache_path: None,
                        default_branch: None,
                        cache_retained: false,
                        status: "failed".to_string(),
                        message: error.to_string(),
                    });
                }
            }
