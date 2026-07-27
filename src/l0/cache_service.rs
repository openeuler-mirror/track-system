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
        }

        Ok(summary)
    }

    async fn warm_package_model(&self, package: packages::Model) -> Result<L0RepoCacheWarmItem> {
        let repo_url = match package
            .l0_repo_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(repo_url) => repo_url.to_string(),
            None => {
                return Ok(L0RepoCacheWarmItem {
                    package_id: package.id,
                    package_name: package.name,
                    repo_url: None,
                    cache_path: None,
                    default_branch: None,
                    cache_retained: false,
                    status: "skipped".to_string(),
                    message: "package missing l0_repo_url".to_string(),
                });
            }
        };

        let package_id = package.id;
        let package_name = package.name.clone();
        let repo_url_for_warm = repo_url.clone();
        let warmed = tokio::task::spawn_blocking(move || warm_cached_mirror(&repo_url_for_warm))
            .await
            .context("join l0 cache warm task failed")??;

        Ok(L0RepoCacheWarmItem {
            package_id,
            package_name,
            repo_url: Some(warmed.repo_url),
            cache_path: Some(warmed.cache_path.display().to_string()),
            default_branch: warmed.default_branch,
            cache_retained: warmed.cache_retained,
            status: "warmed".to_string(),
            message: if warmed.cache_retained {
                "cache warmed".to_string()
            } else {
                "cache warmed and cleaned because retention is disabled".to_string()
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use sea_orm::{DatabaseBackend, MockDatabase};
    use serde_json::json;

    #[tokio::test]
    async fn warm_package_reports_missing_repo_url_as_skipped() {
        let package = packages::Model {
            id: 7,
            name: "openssl".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: None,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let db = MockDatabase::new(DatabaseBackend::Sqlite)
            .append_query_results([[package]])
            .into_connection();
        let service = L0RepoCacheService::new(&db);

        let result = service.warm_package(7).await.unwrap();
        assert_eq!(result.status, "skipped");
        assert_eq!(result.message, "package missing l0_repo_url");
        assert_eq!(result.package_id, 7);
    }

    #[tokio::test]
    async fn warm_all_packages_counts_skipped_entries() {
        let now = Utc::now();
        let db = MockDatabase::new(DatabaseBackend::Sqlite)
            .append_query_results([[
                packages::Model {
                    id: 1,
                    name: "pkg-a".to_string(),
                    level: 1,
                    sync_interval_hours: 24,
                    l0_repo_url: None,
                    description: None,
                    created_at: now,
                    updated_at: Utc::now(),
                },
                packages::Model {
                    id: 2,
                    name: "pkg-b".to_string(),
                    level: 1,
                    sync_interval_hours: 24,
                    l0_repo_url: Some("   ".to_string()),
                    description: Some("test".to_string()),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                },
            ]])
            .into_connection();
        let service = L0RepoCacheService::new(&db);

        let result = service.warm_all_packages().await.unwrap();
        assert_eq!(result.scanned_packages, 2);
        assert_eq!(result.warmed_packages, 0);
        assert_eq!(result.skipped_no_repo, 2);
        assert_eq!(result.failed_packages, 0);
        assert_eq!(result.results.len(), 2);
        assert_eq!(
            serde_json::to_value(&result.results[0]).unwrap()["status"],
            json!("skipped")
        );
    }
}
