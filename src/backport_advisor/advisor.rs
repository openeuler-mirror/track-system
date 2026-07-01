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
                    .filter(backport_candidates::Column::L0CommitId.eq(commit.id))
                    .filter(backport_candidates::Column::TargetDistroId.eq(track.distro_id))
                    .one(self.db)
                    .await?;

                if exists.is_some() {
                    summary.candidates_skipped += 1;
                    continue;
                }

                let recommendation = build_recommendation(
                    &package.name,
                    &distro_map,
                    track.distro_id,
                    &commit.summary,
                    &version,
                );

                let artifact = Some(build_patch_path(&package.name, &commit.commit_sha));

                let candidate = backport_candidates::ActiveModel {
                    package_id: Set(package_id),
                    l0_commit_id: Set(commit.id),
                    target_distro_id: Set(track.distro_id),
                    spec_base_version: Set(version.clone()),
                    recommendation: Set(recommendation),
                    status: Set("pending".to_string()),
                    patch_artifact: Set(artifact),
                    created_at: Set(Utc::now()),
                    updated_at: Set(Utc::now()),
                    ..Default::default()
                };

                candidate.insert(self.db).await?;
                summary.candidates_created += 1;
            }
        }

        Telemetry::backport_candidates_created(
            package_id,
            summary.candidates_created,
            summary.candidates_skipped,
        );
        Ok(summary)
    }
}

fn extract_version(summary: &str) -> Option<String> {
    let regex = Regex::new(r"(\d+\.\d+(?:\.\d+)?)").expect("version regex");
    regex
        .captures(summary)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

fn build_patch_path(package: &str, sha: &str) -> String {
    let short_sha: String = sha.chars().take(8).collect();
    format!("patches/{}-{}.patch", package, short_sha)
}

fn build_recommendation(
    package_name: &str,
    distros: &HashMap<i32, String>,
    distro_id: i32,
    summary: &str,
    version: &str,
) -> String {
    let target = distros
        .get(&distro_id)
        .cloned()
        .unwrap_or_else(|| format!("distro-{}", distro_id));

    format!(
        "建议将 {package} 上游版本 {version} 的提交回合至 {target}。摘要: {summary}",
        package = package_name,
        version = version,
        target = target,
        summary = summary
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DatabaseBackend, MockDatabase};

    #[test]
    fn test_extract_version() {
        assert_eq!(extract_version("Bump to 1.2.3"), Some("1.2.3".to_string()));
        assert!(extract_version("no version here").is_none());
    }

    #[test]
    fn test_build_patch_path() {
        assert_eq!(
            build_patch_path("nginx", "abcdef123456"),
            "patches/nginx-abcdef12.patch"
        );
    }

    #[test]
    fn test_build_recommendation() {
        let mut distros = HashMap::new();
        distros.insert(1, "Fedora 39".to_string());

        let rec = build_recommendation("nginx", &distros, 1, "Fix critical bug", "1.2.3");

        assert!(rec.contains("nginx"));
        assert!(rec.contains("1.2.3"));
        assert!(rec.contains("Fedora 39"));
        assert!(rec.contains("Fix critical bug"));
    }

    #[tokio::test]
    async fn test_generate_for_package_not_found() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results(vec![Vec::<packages::Model>::new()])
            .into_connection();

        let advisor = BackportAdvisor::new(&db);
        let result = advisor.generate_for_package(999).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_generate_for_package_no_tracking() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results(vec![vec![packages::Model {
                id: 1,
                name: "pkg".to_string(),
                level: 0,
                sync_interval_hours: 24,
                l0_repo_url: None,
                description: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }]]) // package found
            .append_query_results(vec![Vec::<tracking::Model>::new()]) // no tracking
            .into_connection();

        let advisor = BackportAdvisor::new(&db);
        let summary = advisor.generate_for_package(1).await.unwrap();
        assert_eq!(summary.candidates_created, 0);
    }
}
