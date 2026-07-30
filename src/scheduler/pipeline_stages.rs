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

//! 流水线各阶段的具体实现

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::ai::{AiAnalysisRequest, AiAnalysisService, AiAnalysisSource, AiContext};
use crate::analyzer::ChangeClassifier;
use crate::backport_advisor::BackportAdvisor;
use crate::diff;
use crate::entities::{l1_commit_records, prelude::*, tracking, tracking_reports};
use crate::metadata_bridge;

use super::pipeline_executor::{
    BackportSuggestionResult, ClassificationResult, ClassifiedCommitResult, DiffComparisonResult,
    L1IngestionResult, L2SnapshotResult, PipelineExecutor, PipelineStage, ReportGenerationResult,
    StageResult,
};
use super::report_artifacts::CveFixComparisonInput;
use super::{SyncService, SyncStatus};

const L2_NEWER_FALLBACK_L2_BRANCHES: [&str; 2] = ["25.05", "25.07"];
const L2_NEWER_PRIMARY_L1_BRANCH: &str = "openEuler-24.03-LTS-SP3";
const L2_NEWER_FALLBACK_L1_BRANCH: &str = "openEuler-24.09";

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum RiskType {
    CodeScan = 1, // 代码扫描漏洞
    CVE = 2,      // CVE 安全漏洞
    Bug = 3,      // bug
    Porting = 4,  // 回合移植
    License = 5,  // license冲突
}

impl serde::Serialize for RiskType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(*self as i32)
    }
}

#[derive(Debug, Clone, Serialize)]
struct RiskCreateReq {
    #[serde(rename = "description")]
    description: String,
    #[serde(rename = "level")]
    level: i32,
    #[serde(rename = "reporter")]
    reporter: String,
    #[serde(rename = "type")]
    r#type: String,
    #[serde(rename = "software")]
    software: String,
    #[serde(rename = "version")]
    version: String,
    #[serde(rename = "release")]
    release: String,
    #[serde(rename = "platform")]
    platform: String,
    #[serde(rename = "disclosure_time", skip_serializing_if = "Option::is_none")]
    disclosure_time: Option<String>,
    #[serde(rename = "source", skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(rename = "package_id")]
    package_id: u64,
    #[serde(rename = "inner_secret")]
    inner_secret: String,
    #[serde(rename = "report_url")]
    report_url: String,
    #[serde(rename = "risk_type")]
    risk_type: RiskType,
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(12).collect()
}

fn infer_changelog_entry_type(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    if lower.contains("cve") || lower.contains("security") || text.contains("安全") {
        "security".to_string()
    } else if lower.contains("fix") || lower.contains("bug") || text.contains("修复") {
        "bugfix".to_string()
    } else {
        "change".to_string()
    }
}

fn latest_version_from_strings(versions: &[String]) -> Option<String> {
    versions
        .iter()
        .filter_map(|version| {
            crate::utils::version::VersionParser::parse(version)
                .ok()
                .filter(|parsed| parsed.major < 1000)
                .map(|parsed| (parsed, version.clone()))
        })
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(_, raw)| raw)
}

fn latest_version_from_tags(
    versions: &[diff::l1_vs_l0::VersionTag],
    stable_only: bool,
) -> Option<String> {
    versions
        .iter()
        .filter(|tag| !stable_only || tag.is_stable)
        .filter_map(|tag| {
            crate::utils::version::VersionParser::parse(&tag.version)
                .ok()
                .map(|parsed| (parsed, tag.version.clone()))
        })
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(_, raw)| raw)
}

async fn latest_snapshot_record(
    db: &sea_orm::DatabaseConnection,
    tracking_id: i32,
    snapshot_type: &str,
) -> Result<Option<crate::entities::l2_snapshots::Model>> {
    use crate::entities::{l2_snapshots, prelude::L2Snapshots};

    L2Snapshots::find()
        .filter(l2_snapshots::Column::TrackingId.eq(tracking_id))
        .filter(l2_snapshots::Column::SnapshotType.eq(snapshot_type))
        .order_by_desc(l2_snapshots::Column::CreatedAt)
        .one(db)
        .await
        .context("查询快照记录失败")
}

async fn latest_l2_snapshot_record_for_tracking(
    db: &sea_orm::DatabaseConnection,
    tracking: &tracking::Model,
) -> Result<Option<crate::entities::l2_snapshots::Model>> {
    use crate::entities::prelude::Tracking;
    use crate::entities::tracking as tracking_entity;

    if !is_l2_newer_fallback_tracking(tracking) {
        return latest_snapshot_record(db, tracking.id, "l2").await;
    }

    let source_tracking = Tracking::find()
        .filter(tracking_entity::Column::PackageId.eq(tracking.package_id))
        .filter(tracking_entity::Column::L2Branch.eq(tracking.l2_branch.clone()))
        .filter(tracking_entity::Column::L1Branch.eq(L2_NEWER_PRIMARY_L1_BRANCH))
        .one(db)
        .await
        .context("查询 fallback L2 快照来源 tracking 失败")?;

    let Some(source_tracking) = source_tracking else {
        warn!(
            tracking_id = tracking.id,
            l2_branch = tracking.l2_branch,
            source_l1_branch = L2_NEWER_PRIMARY_L1_BRANCH,
            "openEuler-24.09 tracking 未找到可复用 L2 快照的 openEuler-24.03 tracking"
        );
        return Ok(None);
    };

    let record = latest_snapshot_record(db, source_tracking.id, "l2").await?;
    if record.is_some() {
        info!(
            tracking_id = tracking.id,
            source_tracking_id = source_tracking.id,
            l2_branch = tracking.l2_branch,
            source_l1_branch = L2_NEWER_PRIMARY_L1_BRANCH,
            "openEuler-24.09 tracking 复用 openEuler-24.03 tracking 的 L2 快照"
        );
    } else {
        warn!(
            tracking_id = tracking.id,
            source_tracking_id = source_tracking.id,
            l2_branch = tracking.l2_branch,
            source_l1_branch = L2_NEWER_PRIMARY_L1_BRANCH,
            "openEuler-24.09 tracking 对应的 openEuler-24.03 tracking 也没有 L2 快照"
        );
    }
    Ok(record)
}

fn is_l2_newer_target_l2_branch(tracking: &tracking::Model) -> bool {
    L2_NEWER_FALLBACK_L2_BRANCHES
        .iter()
        .any(|branch| tracking.l2_branch.contains(branch))
}

fn is_l2_newer_fallback_tracking(tracking: &tracking::Model) -> bool {
    is_l2_newer_target_l2_branch(tracking) && tracking.l1_branch == L2_NEWER_FALLBACK_L1_BRANCH
}

fn l2_newer_fallback_required(
    tracking: &tracking::Model,
    report: &diff::l2_vs_l1::L2VsL1Report,
) -> bool {
    is_l2_newer_target_l2_branch(tracking) && l2_newer_than_l1(report)
}

fn l2_newer_than_l1(report: &diff::l2_vs_l1::L2VsL1Report) -> bool {
    report
        .spec_diff
        .version_diff
        .as_ref()
        .map(|version_diff| {
            version_diff.relationship == diff::l2_vs_l1::VersionRelationship::L2Newer
        })
        .unwrap_or(false)
}

fn l2_vs_l1_diff_summary(r: &diff::l2_vs_l1::L2VsL1Report) -> Value {
    serde_json::json!({
        "patches_added": r.patch_diff.l2_added.len(),
        "patches_added_list": r.patch_diff.l2_added.iter().map(|p| serde_json::json!({
            "filename": p.filename,
            "path": p.path,
            "content_hash": p.content_hash,
            "size": p.size,
            "applied": p.applied,
        })).collect::<Vec<_>>(),
        "patches_modified": r.patch_diff.l2_modified.len(),
        "patches_modified_list": r.patch_diff.l2_modified.iter().map(|p| serde_json::json!({
            "filename": p.filename,
            "l1_hash": p.l1_hash,
            "l2_hash": p.l2_hash,
        })).collect::<Vec<_>>(),
        "patches_removed": r.patch_diff.l2_removed.len(),
        "patches_removed_list": r.patch_diff.l2_removed.iter().map(|p| serde_json::json!({
            "filename": p.filename,
            "path": p.path,
            "content_hash": p.content_hash,
            "size": p.size,
            "applied": p.applied,
        })).collect::<Vec<_>>(),
        "patches_identical": r.patch_diff.identical.len(),
        "has_spec_changes": !r.spec_diff.content_identical,
        "spec_diff": serde_json::json!({
            "version_diff": r.spec_diff.version_diff.as_ref().map(|v| serde_json::json!({
                "l1_version": v.l1_version,
                "l2_version": v.l2_version,
                "relationship": format!("{:?}", v.relationship),
            })),
            "diff_summary": r.spec_diff.diff_summary,
            "key_changes": r.spec_diff.key_changes,
            "build_requires_added": r.spec_diff.build_requires_added,
            "build_requires_removed": r.spec_diff.build_requires_removed,
            "configure_options_added": r.spec_diff.configure_options_added,
            "configure_options_removed": r.spec_diff.configure_options_removed,
        }),
        "conflicts": r.conflicts.len(),
        "commit_diff": serde_json::json!({
            "l1_commits_count": r.commit_diff.l1_commits_count,
            "l2_commits_count": r.commit_diff.l2_commits_count,
            "behind_commits_count": r.commit_diff.behind_commits.len(),
            "behind_commits": r.commit_diff.behind_commits.iter().map(|c| serde_json::json!({
                "sha": c.sha,
                "title": c.title,
                "author": c.author,
                "authored_at": c.authored_at,
                "url": c.url,
                "stats": serde_json::json!({
                    "additions": c.stats.additions,
                    "deletions": c.stats.deletions,
                    "files_changed": c.stats.files_changed,
                }),
                "primary_change_type": c.primary_change_type,
                "cve_list": c.cve_list,
            })).collect::<Vec<_>>(),
            "base_commit": r.commit_diff.base_commit.as_ref().map(|c| serde_json::json!({
                "sha": c.sha,
                "title": c.title,
                "author": c.author,
                "authored_at": c.authored_at,
            })),
            "base_version_release": r.commit_diff.base_version_release,
        }),
    })
}

fn l1_vs_l0_diff_summary(r: &diff::l1_vs_l0::L1VsL0Report) -> Value {
    serde_json::json!({
        "version_behind": r.version_behind,
        "current_version": r.current_version,
        "latest_stable": r.latest_stable,
        "latest_version": r.latest_version,
        "mainline_version": r.mainline_version,
        "upgradable_versions": r.upgradable_versions.len(),
        "upgradable_versions_list": r.upgradable_versions.iter().map(|v| serde_json::json!({
            "version": v.version,
            "release_date": v.release_date,
            "is_security_release": v.is_security_release,
            "breaking_changes": v.breaking_changes,
        })).collect::<Vec<_>>(),
        "patches_merged": r.patch_analysis.merged_in_upstream.len(),
        "patches_merged_list": r.patch_analysis.merged_in_upstream.iter().map(|p| serde_json::json!({
            "filename": p.filename,
            "description": p.description,
            "applied": p.applied,
            "content_hash": p.content_hash,
        })).collect::<Vec<_>>(),
        "patches_still_needed": r.patch_analysis.still_needed.len(),
        "patches_still_needed_list": r.patch_analysis.still_needed.iter().map(|p| serde_json::json!({
            "filename": p.filename,
            "description": p.description,
            "applied": p.applied,
            "content_hash": p.content_hash,
        })).collect::<Vec<_>>(),
        "patches_can_be_removed": r.patch_analysis.can_be_removed_after_upgrade,
        "cves_fixed": r.cve_analysis.fixed_in_upstream.len(),
        "cves_fixed_list": r.cve_analysis.fixed_in_upstream.iter().map(|c| serde_json::json!({
            "cve_id": c.cve_id,
            "patch_file": c.patch_file,
            "description": c.description,
            "severity": c.severity,
        })).collect::<Vec<_>>(),
        "cves_not_fixed": r.cve_analysis.not_fixed_in_upstream.len(),
        "cves_not_fixed_list": r.cve_analysis.not_fixed_in_upstream.iter().map(|c| serde_json::json!({
            "cve_id": c.cve_id,
            "patch_file": c.patch_file,
            "description": c.description,
            "severity": c.severity,
        })).collect::<Vec<_>>(),
        "maintenance_status": serde_json::json!({
            "status": r.maintenance_status.status,
            "stop_maintenance_detected": r.maintenance_status.stop_maintenance_detected,
            "matched_notice": r.maintenance_status.matched_notice,
            "evidence": r.maintenance_status.evidence,
            "confidence": r.maintenance_status.confidence,
        }),
        "outdated_version": serde_json::json!({
            "current_version": r.outdated_version.current_version,
            "latest_version": r.outdated_version.latest_version,
            "latest_version_source": r.outdated_version.latest_version_source,
            "mainline_version": r.outdated_version.mainline_version,
            "mainline_version_source": r.outdated_version.mainline_version_source,
            "major_version_gap": r.outdated_version.major_version_gap,
            "threshold_major_versions": r.outdated_version.threshold_major_versions,
            "is_outdated": r.outdated_version.is_outdated,
        }),
        "lts": serde_json::json!({
            "is_lts": r.lts.is_lts,
            "source": r.lts.source,
            "evidence": r.lts.evidence,
        }),
        "recommendations": r.recommendations,
    })
}

async fn apply_l2_vs_l1_commit_diff(
    db: &sea_orm::DatabaseConnection,
    l2_vs_l1_diff: &Value,
    tracking: &tracking::Model,
    package_name: &str,
    l1_snapshot_version: Option<&str>,
    l1_snapshot_release: Option<&str>,
    base_version: &mut String,
    base_release: &mut String,
    commit_reports: &mut Vec<Value>,
    classification_overrides: &HashMap<String, (String, Vec<String>)>,
    risk_client: Option<&Client>,
    risk_create_url: &str,
) -> Result<()> {
    let Some(commit_diff) = l2_vs_l1_diff.get("commit_diff") else {
        debug!(tracking_id = tracking.id, "commit_diff 为空");
        return Ok(());
    };

    if let Some(version_release) = commit_diff.get("base_version_release") {
        if let Some(version) = version_release.get(0).and_then(Value::as_str) {
            if !version.is_empty() {
                *base_version = version.to_string();
                info!(tracking_id = tracking.id, base_version = %base_version, "获取到 base_commit 版本");
            }
        } else {
            debug!(tracking_id = tracking.id, package_name = %package_name, "version 为空");
        }
        if let Some(release) = version_release.get(1).and_then(Value::as_str) {
            if !release.is_empty() {
                *base_release = release.to_string();
                info!(tracking_id = tracking.id, base_release = %base_release, "获取到 base_commit release");
            }
        } else {
            debug!(tracking_id = tracking.id, package_name = %package_name, "release 为空");
        }
    } else {
        debug!(tracking_id = tracking.id, "base_version_release 为空");
    }

    let Some(behind_commits) = commit_diff.get("behind_commits").and_then(Value::as_array) else {
        return Ok(());
    };
    let behind_commit_shas = behind_commits
        .iter()
        .filter_map(|commit| text_field(commit, "sha"))
        .collect::<Vec<_>>();
    let l1_commits = l1_commit_records_by_sha(db, tracking.id, &behind_commit_shas).await?;
    let fallback_upstream_version_release =
        upstream_version_release_from_diff(l2_vs_l1_diff, l1_snapshot_version, l1_snapshot_release);

    for commit in behind_commits {
        let commit_sha = text_field(commit, "sha").unwrap_or_default();
        let l1_commit = l1_commits.get(&commit_sha);
        let commit_message = l1_commit
            .map(|record| record.commit_message.trim().to_string())
            .filter(|message| !message.is_empty())
            .or_else(|| text_field(commit, "message"))
            .or_else(|| text_field(commit, "title"))
            .unwrap_or_default();
        let (change_type, cve_list) =
            if let Some((change_type, cve_list)) = classification_overrides.get(&commit_sha) {
                (change_type.clone(), cve_list.clone())
            } else {
                let change_type = l1_commit
                    .and_then(|record| record.primary_change_type.clone())
                    .filter(|value| !value.trim().is_empty())
                    .or_else(|| text_field(commit, "primary_change_type"))
                    .unwrap_or_else(|| infer_change_type_from_commit(commit, &commit_message));
                let cve_list = l1_commit
                    .and_then(cve_list_from_l1_record)
                    .unwrap_or_else(|| cve_list_from_json_field(commit, "cve_list"));
                (change_type, cve_list)
            };
        let level = risk_level_label(&change_type);
        let raw_commit_url = l1_commit
            .map(|record| record.api_url.trim().to_string())
            .filter(|url| !url.is_empty())
            .or_else(|| text_field(commit, "url"))
            .unwrap_or_default();
        let commit_url = crate::utils::commit_url::normalize_commit_url_for_branch(
            crate::collectors::traits::Platform::from_str(
                tracking.platform.as_deref().unwrap_or("gitee"),
            )
            .unwrap_or(crate::collectors::traits::Platform::Gitee),
            &raw_commit_url,
            &tracking.l1_branch,
        );
        let author = l1_commit
            .map(|record| record.author_name.trim().to_string())
            .filter(|author| !author.is_empty())
            .or_else(|| text_field(commit, "author"))
            .unwrap_or_default();
        let authored_at = l1_commit
            .map(|record| record.committed_at.to_rfc3339())
            .or_else(|| text_field(commit, "authored_at"));
        let upstream_version_release = commit_version_release(l1_commit)
            .unwrap_or_else(|| fallback_upstream_version_release.clone());

        if let Some(risk_client) = risk_client {
            let req = RiskCreateReq {
                description: format!("{} (branch: {})", commit_message, tracking.l2_branch),
                level: risk_level_number(&change_type),
                reporter: "track-system".to_string(),
                r#type: change_type.clone(),
                software: package_name.to_string(),
                version: base_version.clone(),
                release: base_release.clone(),
                platform: "noarch".to_string(),
                disclosure_time: authored_at.clone(),
                source: Some(tracking.l1_repo_owner.clone()),
                package_id: 0,
                inner_secret: "Ctyun@123".to_string(),
                report_url: commit_url.clone(),
                risk_type: risk_type_from_change_type(&change_type),
            };
            debug!(
                tracking_id = tracking.id,
                commit_sha = %commit_sha,
                req = ?req,
                "调用 risk/create 请求"
            );

            match risk_client
                .post(risk_create_url)
                .header("Content-Type", "application/json")
                .json(&req)
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    let body = resp.text().await.unwrap_or_default();
                    debug!(
                        tracking_id = tracking.id,
                        commit_sha = %commit_sha,
                        body = body,
                        "调用 risk/create 成功"
                    );
                }
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    warn!(
                        tracking_id = tracking.id,
                        commit_sha = %commit_sha,
                        status = status,
                        body = body,
                        "调用 risk/create 失败"
                    );
                }
                Err(err) => {
                    warn!(
                        tracking_id = tracking.id,
                        commit_sha = %commit_sha,
                        error = %err,
                        "调用 risk/create 失败"
                    );
                }
            }
        }

        commit_reports.push(serde_json::json!({
            "Description": commit_message,
            "Level": level,
            "Reporter": author,
            "Software": package_name,
            "Version": base_version,
            "Release": base_release,
            "Platform": "noarch",
            "DisclosureTime": authored_at,
            "Source": &tracking.l1_repo_owner,
            "CommitSha": commit_sha,
            "ChangeType": change_type,
            "CVEList": cve_list,
            "PackageID": tracking.package_id,
            "Url": commit_url,
            "UpstreamVersion": upstream_version_release.0,
            "UpstreamRelease": upstream_version_release.1,
            "UpstreamVersionRelease": version_release_display(&upstream_version_release.0, &upstream_version_release.1),
        }));
    }

    Ok(())
}

async fn l1_commit_records_by_sha(
    db: &sea_orm::DatabaseConnection,
    tracking_id: i32,
    shas: &[String],
) -> Result<HashMap<String, l1_commit_records::Model>> {
    if shas.is_empty() {
        return Ok(HashMap::new());
    }

    let unique_shas = shas
        .iter()
        .map(|sha| sha.trim())
        .filter(|sha| !sha.is_empty())
        .collect::<HashSet<_>>()
        .into_iter()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if unique_shas.is_empty() {
        return Ok(HashMap::new());
    }

    let records = L1CommitRecords::find()
        .filter(l1_commit_records::Column::TrackingId.eq(tracking_id))
        .filter(l1_commit_records::Column::CommitSha.is_in(unique_shas))
        .all(db)
        .await?;

    Ok(records
        .into_iter()
        .map(|record| (record.commit_sha.clone(), record))
        .collect())
}

fn commit_version_release(record: Option<&l1_commit_records::Model>) -> Option<(String, String)> {
    let record = record?;
    let version = record.spec_version.as_deref().unwrap_or_default().trim();
    if version.is_empty() {
        return None;
    }

    let release = record.spec_release.as_deref().unwrap_or_default().trim();
    Some((version.to_string(), release.to_string()))
}

fn cve_list_from_l1_record(record: &l1_commit_records::Model) -> Option<Vec<String>> {
    record
        .cve_list
        .as_ref()
        .map(|value| cve_list_from_value(value))
        .filter(|items| !items.is_empty())
}

fn cve_list_from_json_field(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .map(cve_list_from_value)
        .unwrap_or_default()
}

fn cve_list_from_value(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn upstream_version_release_from_diff(
    l2_vs_l1_diff: &Value,
    l1_snapshot_version: Option<&str>,
    l1_snapshot_release: Option<&str>,
) -> (String, String) {
    let snapshot_version_release = version_release_display(
        l1_snapshot_version.unwrap_or_default(),
        l1_snapshot_release.unwrap_or_default(),
    );
    let version_release = first_non_empty_json_str(&[
        l2_vs_l1_diff.get("l1_version_release"),
        l2_vs_l1_diff
            .get("spec_diff")
            .and_then(|spec| spec.get("version_diff"))
            .and_then(|diff| diff.get("l1_version_release")),
    ]);
    let (version, release) = split_version_release(&version_release);
    if !version.is_empty() && !release.is_empty() {
        return (version, release);
    }

    let version_release_from_diff = first_non_empty_json_str(&[
        l2_vs_l1_diff.get("l1_version"),
        l2_vs_l1_diff
            .get("spec_diff")
            .and_then(|spec| spec.get("version_diff"))
            .and_then(|diff| diff.get("l1_version")),
    ]);
    let version_release =
        first_non_empty_string(&[version_release_from_diff, snapshot_version_release]);
    let (version, release) = split_version_release(&version_release);
    if release.is_empty() {
        (
            version,
            l1_snapshot_release.unwrap_or_default().trim().to_string(),
        )
    } else {
        (version, release)
    }
}

fn first_non_empty_string(values: &[String]) -> String {
    values
        .iter()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn split_version_release(version_release: &str) -> (String, String) {
    let trimmed = version_release.trim();
    if trimmed.is_empty() {
        return (String::new(), String::new());
    }

    match trimmed.rsplit_once('-') {
        Some((version, release)) if !version.trim().is_empty() && !release.trim().is_empty() => {
            (version.trim().to_string(), release.trim().to_string())
        }
        _ => (trimmed.to_string(), String::new()),
    }
}

fn infer_change_type_from_commit(commit: &Value, message: &str) -> String {
    let has_cve = commit
        .get("cve_list")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());
    let lower = message.to_ascii_lowercase();
    if has_cve || lower.contains("cve") || lower.contains("security") {
        "CVE".to_string()
    } else if lower.contains("fix") || lower.contains("bug") || lower.contains("issue") {
        "Bugfix".to_string()
    } else if lower.contains("backport") || lower.contains("cherry-pick") {
        "Backport".to_string()
    } else {
        "Unknown".to_string()
    }
}

fn risk_level_label(change_type: &str) -> &'static str {
    match change_type {
        "CVE" => "High",
        "Bugfix" => "Medium",
        "Unknown" => "Normal",
        _ => "Low",
    }
}

fn risk_level_number(change_type: &str) -> i32 {
    match change_type {
        "CVE" => 3,
        "Bugfix" => 2,
        _ => 1,
    }
}

fn risk_type_from_change_type(change_type: &str) -> RiskType {
    match change_type {
        "CVE" => RiskType::CVE,
        "Bugfix" => RiskType::Bug,
        "Backport" => RiskType::Porting,
        "Unknown" => RiskType::CodeScan,
        _ => RiskType::Porting,
    }
}

fn text_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn first_non_empty_json_str(values: &[Option<&Value>]) -> String {
    values
        .iter()
        .filter_map(|value| value.and_then(Value::as_str))
        .map(str::trim)
        .find(|text| !text.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn classification_overrides(
    classification_result: Option<&StageResult>,
) -> HashMap<String, (String, Vec<String>)> {
    let mut overrides = HashMap::new();
    let Some(class_stage) = classification_result else {
        return overrides;
    };
    let Some(items) = class_stage.details.get("commits").and_then(Value::as_array) else {
        return overrides;
    };

    for item in items {
        let Some(sha) = item.get("commit_sha").and_then(Value::as_str) else {
            continue;
        };
        let change_type = item
            .get("primary_change_type")
            .and_then(Value::as_str)
            .unwrap_or("Unknown")
            .to_string();
        let cve_list = item
            .get("cve_list")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        overrides.insert(sha.to_string(), (change_type, cve_list));
    }

    overrides
}

fn version_release_display(version: &str, release: &str) -> String {
    match (version.trim().is_empty(), release.trim().is_empty()) {
        (true, true) => String::new(),
        (false, true) => version.to_string(),
        (true, false) => release.to_string(),
        (false, false) => format!("{version}-{release}"),
    }
}

fn merge_classification_results_into_commits(
    commit_reports: &mut [Value],
    class_stage: &StageResult,
) {
    let Some(items) = class_stage.details.get("commits").and_then(Value::as_array) else {
        return;
    };

    let mut by_sha: HashMap<String, (&str, &Value)> = HashMap::new();
    for item in items {
        if let Some(sha) = item.get("commit_sha").and_then(Value::as_str) {
            let change_type = item
                .get("primary_change_type")
                .and_then(Value::as_str)
                .unwrap_or("Unknown");
            let cve_list = item.get("cve_list").unwrap_or(&Value::Null);
            by_sha.insert(sha.to_string(), (change_type, cve_list));
        }
    }

    for commit in commit_reports {
        let Some(sha) = commit.get("CommitSha").and_then(Value::as_str) else {
            continue;
        };
        let Some((change_type, cve_list)) = by_sha.get(sha) else {
            continue;
        };
        if let Some(object) = commit.as_object_mut() {
            object.insert(
                "ChangeType".to_string(),
                Value::String((*change_type).to_string()),
            );
            object.insert(
                "Level".to_string(),
                Value::String(risk_level_label(change_type).to_string()),
            );
            object.insert("CVEList".to_string(), (*cve_list).clone());
        }
    }
}

fn append_version_catalog_entries(
    catalog: &Value,
    source: &str,
    collected_at: chrono::DateTime<Utc>,
    seen_versions: &mut HashSet<String>,
    all_versions: &mut Vec<diff::l1_vs_l0::VersionTag>,
    changelogs: &mut HashMap<String, Vec<diff::l1_vs_l0::ChangelogEntry>>,
) {
    let mut candidates = Vec::new();

    if let Some(versions) = catalog.get("versions").and_then(Value::as_array) {
        for item in versions {
            if let Some(version) = version_from_catalog_item(item) {
                candidates.push(version);
            }
        }
    }

    if let Some(latest_stable) = catalog.get("latest_stable").and_then(Value::as_str) {
        candidates.push((latest_stable.to_string(), Some(true), None));
    }
    if let Some(latest_version) = catalog.get("latest_version").and_then(Value::as_str) {
        candidates.push((latest_version.to_string(), None, None));
    }

    for (version, stable_hint, source_ref) in candidates {
        append_l0_version_tag(
            version.trim(),
            stable_hint,
            source_ref.as_deref(),
            source,
            collected_at,
            seen_versions,
            all_versions,
            changelogs,
        );
    }
}

fn version_from_catalog_item(item: &Value) -> Option<(String, Option<bool>, Option<String>)> {
    if let Some(version) = item.as_str() {
        return Some((version.to_string(), None, None));
    }

    let version = item.get("version").and_then(Value::as_str)?.to_string();
    let is_stable = item.get("is_stable").and_then(Value::as_bool);
    let source_ref = item
        .get("source_ref")
        .and_then(Value::as_str)
        .map(str::to_string);
    Some((version, is_stable, source_ref))
}

fn append_l0_version_tag(
    version: &str,
    stable_hint: Option<bool>,
    source_ref: Option<&str>,
    source: &str,
    collected_at: chrono::DateTime<Utc>,
    seen_versions: &mut HashSet<String>,
    all_versions: &mut Vec<diff::l1_vs_l0::VersionTag>,
    changelogs: &mut HashMap<String, Vec<diff::l1_vs_l0::ChangelogEntry>>,
) {
    if version.is_empty() {
        return;
    }

    let Ok(parsed) = crate::utils::version::VersionParser::parse(version) else {
        return;
    };
    if !seen_versions.insert(version.to_string()) {
        return;
    }
    let is_stable = stable_hint.unwrap_or_else(|| parsed.is_stable());
    let source_detail = source_ref
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" ({})", value))
        .unwrap_or_default();

    all_versions.push(diff::l1_vs_l0::VersionTag {
        version: version.to_string(),
        date: collected_at,
        changelog: format!("version catalog: {}{}", source, source_detail),
        is_stable,
    });
    changelogs
        .entry(version.to_string())
        .or_default()
        .push(diff::l1_vs_l0::ChangelogEntry {
            entry_type: "release".to_string(),
            description: format!("版本目录识别到 L0 版本 {}{}", version, source_detail),
            commit_sha: None,
        });
}

fn version_catalog_payloads<'a>(
    evidence: &'a crate::entities::maintenance_evidence_snapshots::Model,
) -> Vec<&'a Value> {
    let mut payloads = Vec::new();
    if let Some(catalog) = find_version_catalog_data(&evidence.raw_payload) {
        payloads.push(catalog);
    }
    if let Some(signals) = evidence.normalized_signals.as_ref() {
        if let Some(catalog) = find_version_catalog_data(signals) {
            payloads.push(catalog);
        }
    }
    payloads
}

fn find_version_catalog_data(value: &Value) -> Option<&Value> {
    if is_version_catalog_data(value) {
        return Some(value);
    }
    value.get("data").and_then(find_version_catalog_data)
}

fn is_version_catalog_data(value: &Value) -> bool {
    value.get("versions").and_then(Value::as_array).is_some()
        || value
            .get("latest_version")
            .and_then(Value::as_str)
            .is_some()
        || value.get("latest_stable").and_then(Value::as_str).is_some()
}

fn should_collect_l0_version_catalog(package: &crate::entities::packages::Model) -> bool {
    let Some(url) = package.l0_repo_url.as_deref() else {
        return false;
    };
    let lower = url.to_ascii_lowercase();

    lower.ends_with(".git")
        || lower.starts_with("git://")
        || lower.starts_with("ssh://")
        || lower.contains('@')
        || lower.contains("github.com/")
        || lower.contains("gitlab.")
        || lower.contains("gitlab.com/")
        || lower.contains("gitee.com/")
        || lower.contains("atomgit.com/")
        || lower.contains("pagure.io/")
}

fn is_patch_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".patch") || lower.ends_with(".diff")
}

fn detect_lts_from_l1_sources(
    tracking: &tracking::Model,
    spec_text: &str,
    commit_messages: &[String],
) -> (Option<bool>, Vec<String>) {
    let mut evidence = Vec::new();
    let branch = tracking.l1_branch.as_str();
    let repo = tracking.l1_repo_name.as_str();

    if contains_lts_positive(branch) {
        evidence.push(format!("L1 分支包含 LTS 标识: {}", branch));
    }
    if contains_lts_positive(repo) {
        evidence.push(format!("L1 仓库名包含 LTS 标识: {}", repo));
    }
    if contains_lts_positive(spec_text) {
        evidence.push("L1 spec 内容包含 LTS/长期维护标识".to_string());
    }
    for message in commit_messages.iter().take(20) {
        if contains_lts_positive(message) {
            evidence.push(format!(
                "L1 commit message 包含 LTS 标识: {}",
                message.chars().take(120).collect::<String>()
            ));
            break;
        }
    }

    evidence.truncate(5);
    if !evidence.is_empty() {
        return (Some(true), evidence);
    }

    let negative_text = format!("{}\n{}\n{}", branch, repo, spec_text);
    if contains_lts_negative(&negative_text) {
        (
            Some(false),
            vec!["L1 仓库信息包含非 LTS/创新版本标识".to_string()],
        )
    } else {
        (None, Vec::new())
    }
}

fn contains_lts_positive(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("lts")
        || lower.contains("long term support")
        || text.contains("长期支持")
        || text.contains("长期维护")
}

fn contains_lts_negative(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("non-lts")
        || lower.contains("non lts")
        || lower.contains("innovation")
        || text.contains("非LTS")
        || text.contains("非 LTS")
        || text.contains("创新版本")
}

fn collect_version_warnings(l1_vs_l0_diff: &Value) -> Vec<String> {
    let mut warnings = Vec::new();
    if let Some(items) = l1_vs_l0_diff
        .get("recommendations")
        .and_then(Value::as_array)
    {
        warnings.extend(
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|item| {
                    item.contains("停维")
                        || item.contains("过时")
                        || item.contains("LTS")
                        || item.contains("长期维护")
                })
                .map(ToString::to_string),
        );
    }
    warnings
}

async fn build_tracking_report_ai_analysis(
    tracking: &tracking::Model,
    package_name: &str,
    diff_summary: &Value,
    representative_changes: &Value,
) -> Value {
    let context = AiContext {
        source: AiAnalysisSource::TrackingReport,
        target_name: Some(package_name.to_string()),
        target_type: Some("package".to_string()),
        platform: tracking.platform.clone(),
        report_type: Some("pipeline".to_string()),
        rule_risk: Some(infer_tracking_report_rule_risk(diff_summary)),
        rule_confidence: Some("medium".to_string()),
        rule_summary: Some(build_tracking_report_rule_summary(
            diff_summary,
            representative_changes,
        )),
        evidence: serde_json::json!({
            "diff_summary": diff_summary,
            "representative_changes": representative_changes,
        }),
    };

    let request = AiAnalysisRequest {
        question: Some(
            "请基于当前 tracking 报告评估 L2 组件相对 L1/L0 的版本、变更和维护风险，并结合 L0 社区安全与质量证据给出处置建议。安全重点关注 L0 仓库是否有安全策略、CVE 修复发布或 CVE 关联记录、待处理 CVE 积压和修复时效；质量重点关注是否有代码 review 机制、专人 review、发布物数字签名、校验值或 provenance/attestation。".to_string(),
        ),
        language: Some("中文".to_string()),
        max_evidence_chars: None,
        allow_external_research: Some(true),
    };

    match AiAnalysisService::from_env()
        .analyze(context, request)
        .await
    {
        Ok(response) => {
            let mut value = serde_json::to_value(response).unwrap_or_else(|error| {
                serde_json::json!({
                    "status": "failed",
                    "generated_at": Utc::now(),
                    "error": format!("序列化 AI 评估结果失败: {}", error),
                })
            });
            if let Some(object) = value.as_object_mut() {
                object.remove("raw_model_output");
            }
            value
        }
        Err(error) => serde_json::json!({
            "status": "failed",
            "generated_at": Utc::now(),
            "error": error.to_string(),
        }),
    }
}

async fn latest_l0_community_assessment(
    db: &sea_orm::DatabaseConnection,
    package: &crate::entities::packages::Model,
) -> Result<Option<Value>> {
    use crate::entities::{ecosystem_reports, ecosystem_targets};

    let mut target_query = EcosystemTargets::find()
        .filter(ecosystem_targets::Column::Role.eq("l0"))
        .filter(ecosystem_targets::Column::Status.eq("active"));

    if let Some(l0_repo_url) = package.l0_repo_url.as_deref() {
        target_query = target_query.filter(
            ecosystem_targets::Column::HomepageUrl
                .eq(l0_repo_url)
                .or(ecosystem_targets::Column::Name.eq(package.name.clone())),
        );
    } else {
        target_query =
            target_query.filter(ecosystem_targets::Column::Name.eq(package.name.clone()));
    }

    let targets = target_query.all(db).await?;
    for target in targets {
        let Some(report) = EcosystemReports::find()
            .filter(ecosystem_reports::Column::TargetId.eq(target.id))
            .order_by_desc(ecosystem_reports::Column::GeneratedAt)
            .one(db)
            .await?
        else {
            continue;
        };

        return Ok(Some(compact_l0_community_assessment(&target, &report)));
    }

    Ok(None)
}

fn compact_l0_community_assessment(
    target: &crate::entities::ecosystem_targets::Model,
    report: &crate::entities::ecosystem_reports::Model,
) -> Value {
    let sections = report.report_payload.get("sections");
    let security = sections.and_then(|value| value.get("security")).cloned();
    let quality = sections.and_then(|value| value.get("quality")).cloned();

    serde_json::json!({
        "target": {
            "id": target.id,
            "name": target.name,
            "role": target.role,
            "platform": target.platform,
            "homepage_url": target.homepage_url,
            "owner": target.owner,
            "repo": target.repo,
        },
        "report": {
            "id": report.id,
            "report_type": report.report_type,
            "overall_risk": report.overall_risk,
            "confidence": report.confidence,
            "summary": report.summary,
            "generated_at": report.generated_at,
        },
        "security": security.map(compact_assessment_section),
        "quality": quality.map(compact_assessment_section),
    })
}

fn compact_assessment_section(section: Value) -> Value {
    let indicators = section
        .get("indicators")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| is_l0_security_quality_indicator(item))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    serde_json::json!({
        "level": section.get("level").cloned().unwrap_or(Value::Null),
        "confidence": section.get("confidence").cloned().unwrap_or(Value::Null),
        "score": section.get("score").cloned().unwrap_or(Value::Null),
        "coverage": section.get("coverage").cloned().unwrap_or(Value::Null),
        "reasons": section.get("reasons").cloned().unwrap_or(Value::Null),
        "evidence_refs": section.get("evidence_refs").cloned().unwrap_or(Value::Null),
        "indicators": indicators,
    })
}

fn is_l0_security_quality_indicator(indicator: &Value) -> bool {
    let Some(key) = indicator.get("key").and_then(Value::as_str) else {
        return false;
    };

    matches!(
        key,
        "has_security_policy"
            | "cve_fix_commits_last_12_months"
            | "cve_linked_issues_last_12_months"
            | "median_cve_fix_days"
            | "open_cve_backlog"
            | "dedicated_code_reviewers"
            | "required_reviews"
            | "signed_releases"
            | "digital_signature_supported"
            | "supports_gpg_commit_tag_verification"
            | "supports_release_attachments"
            | "documented_release_checksum"
            | "documented_release_artifact_signature"
            | "hash_verification_supported"
            | "provenance_attestation"
            | "release_checklist"
            | "hash_signature_assessment"
    )
}

fn infer_tracking_report_rule_risk(diff_summary: &Value) -> String {
    if diff_summary
        .get("l1_vs_l0")
        .and_then(|value| value.get("maintenance_status"))
        .and_then(|value| value.get("stop_maintenance_detected"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || diff_summary
            .get("l1_vs_l0")
            .and_then(|value| value.get("outdated_version"))
            .and_then(|value| value.get("is_outdated"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return "high".to_string();
    }

    let has_high_commit = diff_summary
        .get("commits")
        .and_then(Value::as_array)
        .map(|commits| {
            commits.iter().any(|commit| {
                commit
                    .get("Level")
                    .and_then(Value::as_str)
                    .map(|level| level.eq_ignore_ascii_case("high"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    if has_high_commit {
        return "high".to_string();
    }

    let has_medium_commit = diff_summary
        .get("commits")
        .and_then(Value::as_array)
        .map(|commits| {
            commits.iter().any(|commit| {
                commit
                    .get("Level")
                    .and_then(Value::as_str)
                    .map(|level| level.eq_ignore_ascii_case("medium"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    if has_medium_commit
        || diff_summary
            .get("version_warnings")
            .and_then(Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false)
    {
        return "medium".to_string();
    }

    "low".to_string()
}

fn build_tracking_report_rule_summary(
    diff_summary: &Value,
    representative_changes: &Value,
) -> String {
    let mut parts = Vec::new();
    let behind_commits = diff_summary
        .get("total_behind_commits")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    parts.push(format!("L2 相对 L1 落后 commit 数 {}", behind_commits));

    let version_warning_count = diff_summary
        .get("version_warnings")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    parts.push(format!("版本风险提示 {} 条", version_warning_count));

    if let Some(outdated) = diff_summary
        .get("l1_vs_l0")
        .and_then(|value| value.get("outdated_version"))
    {
        let is_outdated = outdated
            .get("is_outdated")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let gap = outdated
            .get("major_version_gap")
            .and_then(Value::as_i64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        parts.push(format!(
            "过时版本评估 {}，大版本差距 {}",
            if is_outdated { "命中" } else { "未命中" },
            gap
        ));
    }

    let cve_count = representative_changes
        .get("cve_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let needs_review_count = representative_changes
        .get("needs_review_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    parts.push(format!(
        "CVE 数 {}，需人工复核 {}",
        cve_count, needs_review_count
    ));

    parts.join("；")
}

impl<'a> PipelineExecutor<'a> {
    /// 阶段 1: L1 元数据获取
    pub(super) async fn stage_l1_ingestion(
        &self,
        tracking: &tracking::Model,
    ) -> Result<L1IngestionResult> {
        info!(tracking_id = tracking.id, "执行 L1 元数据获取阶段");

        let sync_service = SyncService::new(self.db);

        // 使用新的 Collector 接口进行同步
        // 注意：即使有注入的客户端，我们也使用 sync_tracking，
        // 因为它会根据 tracking 配置自动选择合适的 Collector
        let sync_result = sync_service.sync_tracking(tracking.id).await?;

        // 检查同步状态
        let has_new_data = match sync_result.status {
            SyncStatus::Success => sync_result.commits_synced > 0 || sync_result.issues_synced > 0,
            SyncStatus::Skipped => false,
            SyncStatus::Failed => {
                return Err(anyhow::anyhow!("L1 同步失败: {}", sync_result.message));
            }
        };

        if !has_new_data {
            info!(tracking_id = tracking.id, "L1 没有新数据，可以跳过后续阶段");
        }

        // 如果有新数据，生成并持久化 L1 快照
        let (snapshot_path, snapshot_checksum) = if has_new_data {
            let output_path = format!(
                "/tmp/l1_snapshot_{}_{}.json",
                tracking.id,
                Utc::now().timestamp()
            );

            info!(
                tracking_id = tracking.id,
                repo_path = tracking.l1_repo_name,
                "开始导出 L1 快照"
            );
            let summary =
                metadata_bridge::export_l1_snapshot(self.db, tracking.id, None, &output_path)
                    .await
                    .context("导出 L1 快照失败")?;

            info!(
                tracking_id = tracking.id,
                commit_count = summary.commit_count,
                issue_count = summary.issue_count,
                repo_path = tracking.l1_repo_name,
                "L1 快照生成成功"
            );

            (Some(output_path), Some(summary.checksum))
        } else {
            (None, None)
        };

        Ok(L1IngestionResult {
            commits_synced: sync_result.commits_synced,
            issues_synced: sync_result.issues_synced,
            has_new_data,
            snapshot_path,
            snapshot_checksum,
        })
    }

    /// 阶段 2: L2 快照生成
    pub(super) async fn stage_l2_snapshot(
        &self,
        tracking: &tracking::Model,
    ) -> Result<L2SnapshotResult> {
        info!(
            tracking_id = tracking.id,
            repo_path = tracking.l2_repo_path,
            "执行 L2 快照生成阶段"
        );

        if is_l2_newer_fallback_tracking(tracking) {
            info!(
                tracking_id = tracking.id,
                l1_branch = tracking.l1_branch,
                l2_branch = tracking.l2_branch,
                source_l1_branch = L2_NEWER_PRIMARY_L1_BRANCH,
                "openEuler-24.09 fallback tracking 跳过独立 L2 快照生成，复用 openEuler-24.03 tracking 的 L2 快照"
            );

            let l2_record = latest_l2_snapshot_record_for_tracking(self.db, tracking).await?;
            if let Some(snapshot) = l2_record {
                let snapshot_data: crate::snapshot::types::RepositorySnapshot =
                    serde_json::from_value(snapshot.payload.clone())
                        .context("解析复用 L2 快照 payload 失败")?;

                return Ok(L2SnapshotResult {
                    snapshot_id: Some(snapshot.id as i64),
                    snapshot_path: None,
                    files_count: snapshot_data.files.len(),
                    has_new_data: true,
                });
            }

            warn!(
                tracking_id = tracking.id,
                source_l1_branch = L2_NEWER_PRIMARY_L1_BRANCH,
                "openEuler-24.09 fallback tracking 未找到可复用 L2 快照，跳过独立 L2 快照生成"
            );
            return Ok(L2SnapshotResult {
                snapshot_id: None,
                snapshot_path: None,
                files_count: 0,
                has_new_data: false,
            });
        }

        // 检查 L2 仓库路径是否存在
        let l2_repo_path = PathBuf::from(&tracking.l2_repo_path);
        if !l2_repo_path.exists() {
            warn!(
                tracking_id = tracking.id,
                path = %tracking.l2_repo_path,
                "不存在，尝试使用数据库中的历史快照"
            );

            // 查询数据库中最新的 L2 快照。openEuler-24.09 fallback tracking
            // 不单独采集 L2，复用同 L2 分支 openEuler-24.03 tracking 的 L2 快照。
            let l2_record = latest_l2_snapshot_record_for_tracking(self.db, tracking).await?;

            if let Some(snapshot) = l2_record {
                // 反序列化快照以获取文件数量
                let snapshot_data: crate::snapshot::types::RepositorySnapshot =
                    serde_json::from_value(snapshot.payload.clone())
                        .context("解析 L2 快照 payload 失败")?;

                info!(
                    tracking_id = tracking.id,
                    snapshot_id = snapshot.id,
                    files_count = snapshot_data.files.len(),
                    created_at = %snapshot.created_at,
                    "使用数据库中的历史 L2 快照"
                );

                return Ok(L2SnapshotResult {
                    snapshot_id: Some(snapshot.id as i64),
                    snapshot_path: None,
                    files_count: snapshot_data.files.len(),
                    has_new_data: true,
                });
            } else {
                warn!(
                    tracking_id = tracking.id,
                    "数据库中也不存在 L2 快照，跳过快照生成"
                );
                return Ok(L2SnapshotResult {
                    snapshot_id: None,
                    snapshot_path: None,
                    files_count: 0,
                    has_new_data: false,
                });
            }
        }

        // 生成临时输出路径
        let output_path = format!(
            "/tmp/l2_snapshot_{}_{}.json",
            tracking.id,
            Utc::now().timestamp()
        );

        // 导出 L2 快照
        let summary =
            metadata_bridge::export_l2_snapshot(self.db, tracking.id, &l2_repo_path, &output_path)
                .await
                .context("导出 L2 快照失败")?;

        info!(
            tracking_id = tracking.id,
            files_count = summary.file_count,
            spec_version = ?summary.spec_version,
            "L2 快照生成成功"
        );

        Ok(L2SnapshotResult {
            snapshot_id: None, // metadata_bridge 已经持久化到数据库
            snapshot_path: Some(output_path),
            files_count: summary.file_count,
            has_new_data: true,
        })
    }

    /// 阶段 3: 差异对比
    pub(super) async fn stage_diff_comparison(
        &self,
        tracking: &tracking::Model,
        _previous_results: &HashMap<PipelineStage, StageResult>,
    ) -> Result<DiffComparisonResult> {
        info!(tracking_id = tracking.id, "执行差异对比阶段");

        // 1. 执行 L2 vs L1 对比（内容对比）
        let l2_vs_l1_result = self.compare_l2_vs_l1(tracking).await?;

        // 2. 执行 L1 vs L0 对比（版本对比）
        let l1_vs_l0_result = self.compare_l1_vs_l0(tracking).await?;

        // 3. 保存对比报告到数据库
        let report_id = self
            .save_comparison_reports(tracking, &l2_vs_l1_result, &l1_vs_l0_result)
            .await?;

        let files_changed = l2_vs_l1_result
            .as_ref()
            .map(|r| r.patch_diff.l2_added.len() + r.patch_diff.l2_modified.len())
            .unwrap_or(0);
        let has_spec_changes = l2_vs_l1_result
            .as_ref()
            .map(|r| !r.spec_diff.content_identical)
            .unwrap_or(false);

        info!(
            tracking_id = tracking.id,
            files_changed = files_changed,
            has_spec_changes = has_spec_changes,
            l1_vs_l0_completed = l1_vs_l0_result.is_some(),
            l2_vs_l1_completed = l2_vs_l1_result.is_some(),
            "差异对比完成"
        );

        Ok(DiffComparisonResult {
            report_id: Some(report_id),
            files_changed,
            has_spec_changes,
            l2_vs_l1_diff: l2_vs_l1_result.as_ref().map(l2_vs_l1_diff_summary),
            l1_vs_l0_diff: l1_vs_l0_result.as_ref().map(l1_vs_l0_diff_summary),
        })
    }

    /// 执行 L2 vs L1 对比
    async fn compare_l2_vs_l1(
        &self,
        tracking: &tracking::Model,
    ) -> Result<Option<diff::l2_vs_l1::L2VsL1Report>> {
        use crate::entities::prelude::Packages;
        use sea_orm::EntityTrait;

        info!(tracking_id = tracking.id, "执行 L2 vs L1 对比");

        // 获取软件包名称
        let package = Packages::find_by_id(tracking.package_id)
            .one(self.db)
            .await?
            .context("未找到关联的软件包记录")?;
        let package_name = package.name.clone();

        // 查询最新的 L1/L2 快照
        let l1_record = latest_snapshot_record(self.db, tracking.id, "l1").await?;
        let l2_record = latest_l2_snapshot_record_for_tracking(self.db, tracking).await?;

        if l1_record.is_none() || l2_record.is_none() {
            warn!(
                tracking_id = tracking.id,
                has_l1 = l1_record.is_some(),
                has_l2 = l2_record.is_some(),
                "缺少 L1/L2 快照，跳过内容对比"
            );
            return Ok(None);
        }

        // 反序列化快照
        let l1_snapshot: crate::snapshot::types::RepositorySnapshot =
            serde_json::from_value(l1_record.as_ref().unwrap().payload.clone())
                .context("解析 L1 快照 payload 失败")?;
        let l2_snapshot: crate::snapshot::types::RepositorySnapshot =
            serde_json::from_value(l2_record.as_ref().unwrap().payload.clone())
                .context("解析 L2 快照 payload 失败")?;

        let comparator = diff::l2_vs_l1::L2VsL1Comparator::new();
        let mut l1_snap = diff::l2_vs_l1::L2VsL1Comparator::create_l1_snapshot(
            package_name.clone(),
            &l1_snapshot,
        )
        .context("构建 L1 快照失败")?;
        let l2_snap = diff::l2_vs_l1::L2VsL1Comparator::create_l2_snapshot(
            package_name.clone(),
            &l2_snapshot,
        )
        .context("构建 L2 快照失败")?;

        // 先只判断版本/内容关系。25.05/25.07 可能需要切换到 openEuler-24.09
        // 重新判断，避免在错误的 L1 基线上提前生成误导性的 commit 差异。
        let mut report = comparator
            .compare_with_options(&l1_snap, &l2_snap, self.db, tracking.id, true)
            .await
            .context("L2 vs L1 内容对比预判断失败")?;

        if l2_newer_fallback_required(tracking, &report) {
            let fallback_tracking = self
                .find_or_create_l2_newer_fallback_tracking(tracking)
                .await?;

            if let Some(fallback_tracking) = fallback_tracking {
                if let Some(fallback_l1_record) = self
                    .latest_or_refresh_fallback_l1_snapshot_record(&fallback_tracking)
                    .await?
                {
                    let fallback_l1_snapshot: crate::snapshot::types::RepositorySnapshot =
                        serde_json::from_value(fallback_l1_record.payload.clone())
                            .context("解析 openEuler-24.09 L1 快照 payload 失败")?;
                    let fallback_l1_snap = diff::l2_vs_l1::L2VsL1Comparator::create_l1_snapshot(
                        package_name.clone(),
                        &fallback_l1_snapshot,
                    )
                    .context("构建 openEuler-24.09 L1 快照失败")?;
                    let fallback_probe = comparator
                        .compare_with_options(
                            &fallback_l1_snap,
                            &l2_snap,
                            self.db,
                            fallback_tracking.id,
                            true,
                        )
                        .await
                        .context("使用 openEuler-24.09 重新判断 L2 vs L1 失败")?;

                    if l2_newer_than_l1(&fallback_probe) {
                        info!(
                            tracking_id = tracking.id,
                            fallback_tracking_id = fallback_tracking.id,
                            fallback_l1_branch = L2_NEWER_FALLBACK_L1_BRANCH,
                            "L2 版本仍高于 openEuler-24.09，跳过 commit 差异判断"
                        );
                        l1_snap = fallback_l1_snap;
                        report = fallback_probe;
                    } else {
                        info!(
                            tracking_id = tracking.id,
                            fallback_tracking_id = fallback_tracking.id,
                            fallback_l1_branch = L2_NEWER_FALLBACK_L1_BRANCH,
                            "使用 openEuler-24.09 重新判断后 L2 不再领先，采用 openEuler-24.09 对比结果"
                        );
                        report = comparator
                            .compare_with_commit_tracking_ids(
                                &fallback_l1_snap,
                                &l2_snap,
                                self.db,
                                fallback_tracking.id,
                                tracking.id,
                                false,
                            )
                            .await
                            .context("使用 openEuler-24.09 重新执行 L2 vs L1 内容对比失败")?;
                        l1_snap = fallback_l1_snap;
                    }
                } else {
                    warn!(
                        tracking_id = tracking.id,
                        fallback_tracking_id = fallback_tracking.id,
                        fallback_l1_branch = L2_NEWER_FALLBACK_L1_BRANCH,
                        "openEuler-24.09 L1 快照不可用，跳过 commit 差异判断避免误判"
                    );
                }
            } else {
                warn!(
                    tracking_id = tracking.id,
                    l2_branch = tracking.l2_branch,
                    fallback_l1_branch = L2_NEWER_FALLBACK_L1_BRANCH,
                    "未找到 openEuler-24.09 tracking，跳过 commit 差异判断避免误判"
                );
            }
        } else {
            if l2_newer_than_l1(&report) {
                info!(
                    tracking_id = tracking.id,
                    l2_branch = tracking.l2_branch,
                    "L2 版本高于 L1，认为 L2 没有落后 commit，跳过 commit 差异判断"
                );
            } else {
                report = comparator
                    .compare(&l1_snap, &l2_snap, self.db, tracking.id)
                    .await
                    .context("L2 vs L1 内容对比失败")?;
            }
        }

        info!(
            tracking_id = tracking.id,
            l1_patches = l1_snap.patches.len(),
            l2_patches = l2_snap.patches.len(),
            has_spec_changes = !report.spec_diff.content_identical,
            "L2 vs L1 对比完成"
        );

        Ok(Some(report))
    }

    async fn latest_or_refresh_fallback_l1_snapshot_record(
        &self,
        fallback_tracking: &tracking::Model,
    ) -> Result<Option<crate::entities::l2_snapshots::Model>> {
        if let Some(record) = latest_snapshot_record(self.db, fallback_tracking.id, "l1").await? {
            return Ok(Some(record));
        }

        info!(
            tracking_id = fallback_tracking.id,
            fallback_l1_branch = L2_NEWER_FALLBACK_L1_BRANCH,
            "openEuler-24.09 L1 快照缺失，尝试即时同步并生成快照"
        );

        let sync_service = SyncService::new(self.db);
        match sync_service.sync_tracking(fallback_tracking.id).await {
            Ok(sync_result) => {
                info!(
                    tracking_id = fallback_tracking.id,
                    commits_synced = sync_result.commits_synced,
                    issues_synced = sync_result.issues_synced,
                    status = ?sync_result.status,
                    "openEuler-24.09 L1 即时同步完成"
                );
            }
            Err(err) => {
                warn!(
                    tracking_id = fallback_tracking.id,
                    error = %err,
                    "openEuler-24.09 L1 即时同步失败"
                );
                return Ok(None);
            }
        }

        let output_path = format!(
            "/tmp/l1_fallback_snapshot_{}_{}.json",
            fallback_tracking.id,
            Utc::now().timestamp()
        );
        if let Err(err) =
            metadata_bridge::export_l1_snapshot(self.db, fallback_tracking.id, None, &output_path)
                .await
        {
            warn!(
                tracking_id = fallback_tracking.id,
                error = %err,
                "openEuler-24.09 L1 即时快照生成失败"
            );
            return Ok(None);
        }

        latest_snapshot_record(self.db, fallback_tracking.id, "l1").await
    }

    async fn find_or_create_l2_newer_fallback_tracking(
        &self,
        source_tracking: &tracking::Model,
    ) -> Result<Option<tracking::Model>> {
        use crate::entities::tracking as tracking_entity;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        if source_tracking.l1_branch == L2_NEWER_FALLBACK_L1_BRANCH {
            return Ok(Some(source_tracking.clone()));
        }

        if let Some(existing) = Tracking::find()
            .filter(tracking_entity::Column::PackageId.eq(source_tracking.package_id))
            .filter(tracking_entity::Column::L2Branch.eq(source_tracking.l2_branch.clone()))
            .filter(tracking_entity::Column::L1RepoOwner.eq(source_tracking.l1_repo_owner.clone()))
            .filter(tracking_entity::Column::L1RepoName.eq(source_tracking.l1_repo_name.clone()))
            .filter(tracking_entity::Column::L1Branch.eq(L2_NEWER_FALLBACK_L1_BRANCH))
            .one(self.db)
            .await
            .context("查询 openEuler-24.09 fallback tracking 失败")?
        {
            return Ok(Some(existing));
        }

        let now = Utc::now();
        let fallback = tracking::ActiveModel {
            package_id: Set(source_tracking.package_id),
            distro_id: Set(source_tracking.distro_id),
            l1_repo_owner: Set(source_tracking.l1_repo_owner.clone()),
            l1_repo_name: Set(source_tracking.l1_repo_name.clone()),
            l1_branch: Set(L2_NEWER_FALLBACK_L1_BRANCH.to_string()),
            l2_branch: Set(source_tracking.l2_branch.clone()),
            l2_repo_path: Set(source_tracking.l2_repo_path.clone()),
            tracking_status: Set("active".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
            platform: Set(source_tracking.platform.clone()),
            ..Default::default()
        };

        match fallback.insert(self.db).await {
            Ok(model) => {
                info!(
                    tracking_id = source_tracking.id,
                    fallback_tracking_id = model.id,
                    fallback_l1_branch = L2_NEWER_FALLBACK_L1_BRANCH,
                    "按需创建 openEuler-24.09 fallback tracking"
                );
                Ok(Some(model))
            }
            Err(err) => {
                warn!(
                    tracking_id = source_tracking.id,
                    error = %err,
                    fallback_l1_branch = L2_NEWER_FALLBACK_L1_BRANCH,
                    "按需创建 openEuler-24.09 fallback tracking 失败"
                );
                Err(err).context("创建 openEuler-24.09 fallback tracking 失败")
            }
        }
    }

    /// 执行 L1 vs L0 对比
    async fn compare_l1_vs_l0(
        &self,
        tracking: &tracking::Model,
    ) -> Result<Option<diff::l1_vs_l0::L1VsL0Report>> {
        use crate::diff::l1_vs_l0::L1VsL0Comparator;

        info!(tracking_id = tracking.id, "执行 L1 vs L0 对比");

        // 获取 L0 版本信息（从 l0_commits 表）
        let l0_info = self.get_l0_version_info(tracking).await?;

        // 获取 L1 版本信息（从 commit_records 和快照）
        let l1_info = self.get_l1_version_info(tracking).await?;
        let Some(l1_info) = l1_info else {
            warn!(
                tracking_id = tracking.id,
                "缺少 L1 版本信息，跳过 L1 vs L0 对比"
            );
            return Ok(None);
        };

        // 使用 L1VsL0Comparator
        let comparator = L1VsL0Comparator::new();
        let report = if let Some(l0_info) = l0_info {
            comparator
                .compare(&l0_info, &l1_info)
                .await
                .context("L1 vs L0 对比失败")?
        } else {
            warn!(
                tracking_id = tracking.id,
                "缺少 L0 版本信息，生成部分 L1 vs L0 评估"
            );
            comparator.compare_without_l0(&l1_info)
        };

        info!(
            tracking_id = tracking.id,
            version_behind = report.version_behind,
            "L1 vs L0 对比完成"
        );

        Ok(Some(report))
    }

    /// 获取 L0 版本信息
    async fn get_l0_version_info(
        &self,
        tracking: &tracking::Model,
    ) -> Result<Option<diff::l1_vs_l0::L0VersionInfo>> {
        use crate::entities::{l0_commits, prelude::*};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        // 从 l0_commits 表获取版本信息
        let l0_commits = L0Commits::find()
            .filter(l0_commits::Column::PackageId.eq(tracking.package_id))
            .order_by_desc(l0_commits::Column::AuthoredAt)
            .all(self.db)
            .await?
            .into_iter()
            .take(100)
            .collect::<Vec<_>>();

        if l0_commits.is_empty() {
            return Ok(None);
        }

        // TODO: 从 l0_commits 构建 L0VersionInfo
        // 这里需要解析 commit message 和 tags 来提取版本信息
        // 暂时返回 None，需要进一步实现
        warn!(tracking_id = tracking.id, "L0 版本信息提取功能待实现");

        Ok(None)
    }

    /// 获取 L1 版本信息
    async fn get_l1_version_info(
        &self,
        tracking: &tracking::Model,
    ) -> Result<Option<diff::l1_vs_l0::L1VersionInfo>> {
        // TODO: 从 commit_records 和快照提取 L1 版本信息
        // 需要解析 spec 文件和 patch 文件
        warn!(tracking_id = tracking.id, "L1 版本信息提取功能待实现");

        Ok(None)
    }

    /// 保存对比报告
    async fn save_comparison_reports(
        &self,
        tracking: &tracking::Model,
        l2_vs_l1: &Option<diff::l2_vs_l1::L2VsL1Report>,
        l1_vs_l0: &Option<diff::l1_vs_l0::L1VsL0Report>,
    ) -> Result<i64> {
        use sea_orm::Set;

        // 构建报告摘要，包含详细的 patch 和 commit_diff 信息

        let l2_vs_l1_diff = l2_vs_l1.as_ref().map(|r| serde_json::json!({
                "patches_added": r.patch_diff.l2_added.len(),
                "patches_added_list": r.patch_diff.l2_added.iter().map(|p| serde_json::json!({
                    "filename": p.filename,
                    "path": p.path,
                    "content_hash": p.content_hash,
                    "size": p.size,
                    "applied": p.applied,
                })).collect::<Vec<_>>(),
                "patches_modified": r.patch_diff.l2_modified.len(),
                "patches_modified_list": r.patch_diff.l2_modified.iter().map(|p| serde_json::json!({
                    "filename": p.filename,
                    "l1_hash": p.l1_hash,
                    "l2_hash": p.l2_hash,
                })).collect::<Vec<_>>(),
                "patches_removed": r.patch_diff.l2_removed.len(),
                "patches_removed_list": r.patch_diff.l2_removed.iter().map(|p| serde_json::json!({
                    "filename": p.filename,
                    "path": p.path,
                    "content_hash": p.content_hash,
                    "size": p.size,
                    "applied": p.applied,
                })).collect::<Vec<_>>(),
                "patches_identical": r.patch_diff.identical.len(),
                "has_spec_changes": !r.spec_diff.content_identical,
                "spec_diff": serde_json::json!({
                    "version_diff": r.spec_diff.version_diff.as_ref().map(|v| serde_json::json!({
                        "l1_version": v.l1_version,
                        "l2_version": v.l2_version,
                        "relationship": format!("{:?}", v.relationship),
                    })),
                    "diff_summary": r.spec_diff.diff_summary,
                    "key_changes": r.spec_diff.key_changes,
                    "build_requires_added": r.spec_diff.build_requires_added,
                    "build_requires_removed": r.spec_diff.build_requires_removed,
                    "configure_options_added": r.spec_diff.configure_options_added,
                    "configure_options_removed": r.spec_diff.configure_options_removed,
                }),
                "conflicts": r.conflicts.len(),
                "commit_diff": serde_json::json!({
                    "l1_commits_count": r.commit_diff.l1_commits_count,
                    "l2_commits_count": r.commit_diff.l2_commits_count,
                    "behind_commits_count": r.commit_diff.behind_commits.len(),
                    "behind_commits": r.commit_diff.behind_commits.iter().map(|c| serde_json::json!({
                        "sha": c.sha,
                        "title": c.title,
                        "author": c.author,
                        "authored_at": c.authored_at,
                        "url": c.url,
                        "stats": serde_json::json!({
                            "additions": c.stats.additions,
                            "deletions": c.stats.deletions,
                            "files_changed": c.stats.files_changed,
                        }),
                        "primary_change_type": c.primary_change_type,
                        "cve_list": c.cve_list,
                    })).collect::<Vec<_>>(),
                    "base_commit": r.commit_diff.base_commit.as_ref().map(|c| serde_json::json!({
                        "sha": c.sha,
                        "title": c.title,
                        "author": c.author,
                        "authored_at": c.authored_at,
                    })),
                    "base_version_release": r.commit_diff.base_version_release,
                }),
            }));

        let l1_vs_l0_diff = l1_vs_l0.as_ref().map(|r| serde_json::json!({
                "version_behind": r.version_behind,
                "current_version": r.current_version,
                "latest_stable": r.latest_stable,
                "latest_version": r.latest_version,
                "upgradable_versions": r.upgradable_versions.len(),
                "upgradable_versions_list": r.upgradable_versions.iter().map(|v| serde_json::json!({
                    "version": v.version,
                    "release_date": v.release_date,
                    "is_security_release": v.is_security_release,
                    "breaking_changes": v.breaking_changes,
                })).collect::<Vec<_>>(),
                "patches_merged": r.patch_analysis.merged_in_upstream.len(),
                "patches_merged_list": r.patch_analysis.merged_in_upstream.iter().map(|p| serde_json::json!({
                    "filename": p.filename,
                    "description": p.description,
                    "applied": p.applied,
                    "content_hash": p.content_hash,
                })).collect::<Vec<_>>(),
                "patches_still_needed": r.patch_analysis.still_needed.len(),
                "patches_still_needed_list": r.patch_analysis.still_needed.iter().map(|p| serde_json::json!({
                    "filename": p.filename,
                    "description": p.description,
                    "applied": p.applied,
                    "content_hash": p.content_hash,
                })).collect::<Vec<_>>(),
                "patches_can_be_removed": r.patch_analysis.can_be_removed_after_upgrade,
                "cves_fixed": r.cve_analysis.fixed_in_upstream.len(),
                "cves_fixed_list": r.cve_analysis.fixed_in_upstream.iter().map(|c| serde_json::json!({
                    "cve_id": c.cve_id,
                    "patch_file": c.patch_file,
                    "description": c.description,
                    "severity": c.severity,
                })).collect::<Vec<_>>(),
                "cves_not_fixed": r.cve_analysis.not_fixed_in_upstream.len(),
                "cves_not_fixed_list": r.cve_analysis.not_fixed_in_upstream.iter().map(|c| serde_json::json!({
                    "cve_id": c.cve_id,
                    "patch_file": c.patch_file,
                    "description": c.description,
                    "severity": c.severity,
                })).collect::<Vec<_>>(),
                "recommendations": r.recommendations,
            }));

        // 创建报告记录
        let report = crate::entities::compare_reports::ActiveModel {
            tracking_id: Set(tracking.id),
            generated_at: Set(Utc::now()),
            l2_vs_l1_diff: Set(l2_vs_l1_diff),
            l1_vs_l0_diff: Set(l1_vs_l0_diff),
            status: Set("success".to_string()),
            failure_reason: Set(None),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        };

        let inserted = report.insert(self.db).await.context("插入报告记录失败")?;

        Ok(inserted.id as i64)
    }

    /// 阶段 4: 变更分类
    pub(super) async fn stage_classification(
        &self,
        tracking: &tracking::Model,
        _previous_results: &HashMap<PipelineStage, StageResult>,
    ) -> Result<ClassificationResult> {
        info!(tracking_id = tracking.id, "执行变更分类阶段");

        // 获取所有待分类的 commits（classification_status = 'pending'）
        let pending_commits = L1CommitRecords::find()
            .filter(l1_commit_records::Column::TrackingId.eq(tracking.id))
            .filter(l1_commit_records::Column::ClassificationStatus.eq("pending"))
            .all(self.db)
            .await
            .context("查询待分类 commits 失败")?;

        if pending_commits.is_empty() {
            info!(tracking_id = tracking.id, "没有待分类的 commits");
            return Ok(ClassificationResult {
                classified_count: 0,
                cve_count: 0,
                needs_review_count: 0,
                commits: Vec::new(),
            });
        }

        let classifier = ChangeClassifier::new(self.db);
        let mut classified_count = 0;
        let mut cve_count = 0;
        let mut needs_review_count = 0;

        for commit in pending_commits {
            // 分类 commit
            match classifier.classify_commit(commit.id).await {
                Ok(classification) => {
                    // 更新 commit 记录
                    let mut active_commit: l1_commit_records::ActiveModel = commit.into();
                    active_commit.primary_change_type =
                        Set(Some(classification.primary_type.as_str().to_string()));
                    active_commit.cve_list =
                        Set(Some(serde_json::to_value(&classification.cve_numbers)?));
                    active_commit.spec_changed = Set(classification.has_spec_change);
                    active_commit.classification_status = Set("done".to_string());
                    active_commit.updated_at = Set(Utc::now());

                    active_commit
                        .update(self.db)
                        .await
                        .context("更新 commit 分类失败")?;

                    classified_count += 1;
                    cve_count += classification.cve_numbers.len();

                    // 检查是否需要人工审核
                    if classification.primary_type.as_str() == "MixedChange" {
                        needs_review_count += 1;
                    }
                }
                Err(err) => {
                    warn!(
                        commit_id = commit.id,
                        error = %err,
                        "分类 commit 失败"
                    );
                }
            }
        }

        info!(
            tracking_id = tracking.id,
            classified_count = classified_count,
            cve_count = cve_count,
            needs_review_count = needs_review_count,
            "变更分类完成"
        );

        Ok(ClassificationResult {
            classified_count,
            cve_count,
            needs_review_count,
            commits: Vec::new(),
        })
    }

    /// 阶段 5: 报告生成
    pub(super) async fn stage_report_generation(
        &self,
        tracking: &tracking::Model,
        previous_results: &HashMap<PipelineStage, StageResult>,
    ) -> Result<ReportGenerationResult> {
        info!(tracking_id = tracking.id, "执行报告生成阶段");

        // 从前面的阶段结果中提取信息
        let diff_result = previous_results.get(&PipelineStage::DiffComparison);
        let classification_result = previous_results.get(&PipelineStage::Classification);

        // 获取 package 信息
        use crate::entities::prelude::*;
        let package = Packages::find_by_id(tracking.package_id)
            .one(self.db)
            .await?
            .context("未找到关联的软件包记录")?;
        let package_name = package.name.clone();

        let risk_create_url = std::env::var("RISK_CREATE_URL")
            .unwrap_or_else(|_| "http://localhost:8899/risk/create/inner".to_string());
        let risk_create_enabled = std::env::var("RISK_CREATE_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .to_lowercase()
            != "false";

        let risk_timeout_secs: u64 = std::env::var("RISK_HTTP_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        let risk_client = if risk_create_enabled {
            Some(
                Client::builder()
                    .timeout(Duration::from_secs(risk_timeout_secs))
                    .build()
                    .context("创建 Risk HTTP 客户端失败")?,
            )
        } else {
            None
        };

        // 存储每个commit的独立信息
        let mut commit_reports = Vec::new();
        let mut base_version = String::new();
        let mut base_release = String::new();

        // 从 diff_result 中获取 report_id，然后查询 compare_reports 表
        if let Some(diff_stage) = diff_result {
            if let Some(report_id) = diff_stage.details.get("report_id").and_then(|v| v.as_i64()) {
                // 查询 compare_reports 表获取对比数据
                if let Some(compare_report) = CompareReports::find_by_id(report_id as i32)
                    .one(self.db)
                    .await?
                {
                    // 从 l2_vs_l1_diff 中提取 commit_diff 信息
                    if let Some(l2_vs_l1_diff) = &compare_report.l2_vs_l1_diff {
                        if let Some(commit_diff) = l2_vs_l1_diff.get("commit_diff") {
                            // 获取 base_version_release
                            if let Some(version_release) = commit_diff.get("base_version_release") {
                                if let Some(version) =
                                    version_release.get(0).and_then(|v| v.as_str())
                                {
                                    base_version = version.to_string();
                                    info!(tracking_id = tracking.id, base_version = %base_version, "获取到 base_commit 版本");
                                }
                                if let Some(release) =
                                    version_release.get(1).and_then(|v| v.as_str())
                                {
                                    base_release = release.to_string();
                                    info!(tracking_id = tracking.id, base_release = %base_release, "获取到 base_commit release");
                                }
                            }

                            // 获取 behind_commits 列表
                            if let Some(behind_commits) =
                                commit_diff.get("behind_commits").and_then(|v| v.as_array())
                            {
                                // 提取 behind_commits 中的 SHA 列表
                                let behind_commit_shas: Vec<String> = behind_commits
                                    .iter()
                                    .filter_map(|c| {
                                        c.get("sha").and_then(|s| s.as_str()).map(|s| s.to_string())
                                    })
                                    .collect();

                                // 从 l1_commit_records 表中获取这些 commit 的详细信息
                                if !behind_commit_shas.is_empty() {
                                    let commits = L1CommitRecords::find()
                                        .filter(
                                            l1_commit_records::Column::TrackingId.eq(tracking.id),
                                        )
                                        .filter(
                                            l1_commit_records::Column::CommitSha
                                                .is_in(behind_commit_shas),
                                        )
                                        .all(self.db)
                                        .await?;

                                    // 为每个 commit 创建独立的信息记录
                                    for commit in commits {
                                        // 根据 primary_change_type 判断 level
                                        let level = match commit.primary_change_type.as_deref() {
                                            Some("CVE") => "High",
                                            Some("Bugfix") => "Medium",
                                            Some(_) => "Low",
                                            None => "Normal",
                                        };

                                        if let Some(risk_client) = &risk_client {
                                            let risk_level =
                                                match commit.primary_change_type.as_deref() {
                                                    Some("CVE") => 3,
                                                    Some("Bugfix") => 2,
                                                    Some(_) => 1,
                                                    None => 1,
                                                };

                                            let version = if base_version.is_empty() {
                                                commit
                                                    .spec_version
                                                    .clone()
                                                    .unwrap_or_else(|| "unknown".to_string())
                                            } else {
                                                base_version.clone()
                                            };

                                            let release = if base_release.is_empty() {
                                                commit
                                                    .spec_release
                                                    .clone()
                                                    .unwrap_or_else(|| "unknown".to_string())
                                            } else {
                                                base_release.clone()
                                            };

                                            let req = RiskCreateReq {
                                                description: format!(
                                                    "{}\n{}",
                                                    commit.commit_message, commit.api_url
                                                ),
                                                level: risk_level,
                                                reporter: "track-system".to_string(),
                                                r#type: commit
                                                    .primary_change_type
                                                    .clone()
                                                    .unwrap_or_else(|| "Unknown".to_string()),
                                                software: package_name.clone(),
                                                version,
                                                release,
                                                platform: "noarch".to_string(),
                                                disclosure_time: Some(
                                                    commit.committed_at.to_rfc3339(),
                                                ),
                                                source: Some(tracking.l1_repo_owner.clone()),
                                                package_id: 0,
                                                inner_secret: "Ctyun@123".to_string(),
                                                report_url: commit.api_url.clone(),
                                                risk_type: risk_type_from_change_type(
                                                    &commit
                                                        .primary_change_type
                                                        .clone()
                                                        .unwrap_or_else(|| "Unknown".to_string()),
                                                ),
                                            };
                                            info!(
                                                tracking_id = tracking.id,
                                                commit_sha = %commit.commit_sha,
                                                req = ?req,
                                                "调用 risk/create 请求"
                                            );

                                            match risk_client
                                                .post(&risk_create_url)
                                                .header("Content-Type", "application/json")
                                                .json(&req)
                                                .send()
                                                .await
                                            {
                                                Ok(resp) if resp.status().is_success() => {
                                                    let body =
                                                        resp.text().await.unwrap_or_default();
                                                    info!(
                                                        tracking_id = tracking.id,
                                                        commit_sha = %commit.commit_sha,
                                                        body = body,
                                                        "调用 risk/create 成功"
                                                    );
                                                }
                                                Ok(resp) => {
                                                    let status = resp.status().as_u16();
                                                    let body =
                                                        resp.text().await.unwrap_or_default();
                                                    warn!(
                                                        tracking_id = tracking.id,
                                                        commit_sha = %commit.commit_sha,
                                                        status = status,
                                                        body = body,
                                                        "调用 risk/create 失败"
                                                    );
                                                }
                                                Err(err) => {
                                                    warn!(
                                                        tracking_id = tracking.id,
                                                        commit_sha = %commit.commit_sha,
                                                        error = %err,
                                                        "调用 risk/create 失败"
                                                    );
                                                }
                                            }
                                        }

                                        // 构建单个commit的信息
                                        let commit_info = serde_json::json!({
                                            "Description": commit.commit_message,
                                            "Level": level,
                                            "Reporter": commit.author_name,
                                            "Software": &package_name,
                                            "Version": &base_version,
                                            "Release": &base_release,
                                            "Platform": "noarch",
                                            "DisclosureTime": commit.committed_at.to_rfc3339(),
                                            "Source": &tracking.l1_repo_owner,
                                            "CommitSha": commit.commit_sha,
                                            "ChangeType": commit.primary_change_type.unwrap_or_else(|| "Unknown".to_string()),
                                            "CVEList": commit.cve_list.unwrap_or_else(|| serde_json::json!([])),
                                            "PackageID": tracking.package_id,
                                            "Url": commit.api_url,
                                        });
                                        commit_reports.push(commit_info);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 构建报告摘要 - 使用commit_reports数组
        let diff_summary = serde_json::json!({
            "commits": commit_reports,
            "total_behind_commits": commit_reports.len(),
            "tracking_id": tracking.id,
            "package_name": package_name,
        });

        // 从 classification_result 提取统计信息
        let representative_changes = if let Some(class_stage) = classification_result {
            if let Some(details) = class_stage.details.as_object() {
                serde_json::json!({
                    "classified_count": details.get("classified_count").and_then(|v| v.as_u64()).unwrap_or(0),
                    "cve_count": details.get("cve_count").and_then(|v| v.as_u64()).unwrap_or(0),
                    "needs_review_count": details.get("needs_review_count").and_then(|v| v.as_u64()).unwrap_or(0),
                })
            } else {
                serde_json::json!({
                    "classified_count": 0,
                    "cve_count": 0,
                    "needs_review_count": 0,
                })
            }
        } else {
            serde_json::json!({
                "classified_count": 0,
                "cve_count": 0,
                "needs_review_count": 0,
            })
        };

        // 创建报告记录到 tracking_reports 表（用于最终报告）
        let report = tracking_reports::ActiveModel {
            tracking_id: Set(tracking.id),
            generated_at: Set(Utc::now()),
            diff_summary: Set(diff_summary),
            representative_changes: Set(Some(representative_changes)),
            source: Set("pipeline".to_string()),
            status: Set("success".to_string()),
            failure_reason: Set(None),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        };

        let inserted = report.insert(self.db).await.context("插入报告记录失败")?;

        info!(
            tracking_id = tracking.id,
            report_id = inserted.id,
            commits_count = commit_reports.len(),
            "报告生成成功"
        );

        Ok(ReportGenerationResult {
            report_id: inserted.id as i64,
            report_status: "success".to_string(),
            cve_fix_comparison_input: None,
            cve_fix_comparison_xlsx: None,
        })
    }

    /// 阶段 6: 回合建议
    pub(super) async fn stage_backport_suggestion(
        &self,
        tracking: &tracking::Model,
        _previous_results: &HashMap<PipelineStage, StageResult>,
    ) -> Result<BackportSuggestionResult> {
        info!(tracking_id = tracking.id, "执行回合建议阶段");

        // 获取 package_id
        let package_id = tracking.package_id;

        // 使用 BackportAdvisor 生成回合候选
        let advisor = BackportAdvisor::new(self.db);
        let summary = advisor
            .generate_for_package(package_id)
            .await
            .context("生成回合候选失败")?;

        info!(
            tracking_id = tracking.id,
            candidates_created = summary.candidates_created,
            candidates_skipped = summary.candidates_skipped,
            "回合建议生成完成"
        );

        Ok(BackportSuggestionResult {
            candidates_count: summary.candidates_created,
            l0_commits_checked: summary.candidates_created + summary.candidates_skipped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff;
    use crate::entities::{l0_commits, l2_snapshots, packages, tracking};
    use chrono::Utc;
    use sea_orm::{DatabaseBackend, MockDatabase};
    use serial_test::serial;
    use std::env;

    struct EnvVarGuard {
        key: String,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &str, value: &str) -> Self {
            let previous = env::var(key).ok();
            env::set_var(key, value);
            Self {
                key: key.to_string(),
                previous,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => env::set_var(&self.key, v),
                None => env::remove_var(&self.key),
            }
        }
    }

    #[test]
    fn test_l1_ingestion_result_has_new_data() {
        let result = L1IngestionResult {
            commits_synced: 5,
            issues_synced: 0,
            has_new_data: true,
            snapshot_path: Some("/tmp/test.json".to_string()),
            snapshot_checksum: Some("abc123".to_string()),
        };

        assert_eq!(result.commits_synced, 5);
        assert_eq!(result.issues_synced, 0);
        assert!(result.has_new_data);
        assert!(result.snapshot_path.is_some());
        assert!(result.snapshot_checksum.is_some());
    }

    #[test]
    fn test_l1_ingestion_result_no_new_data() {
        let result = L1IngestionResult {
            commits_synced: 0,
            issues_synced: 0,
            has_new_data: false,
            snapshot_path: None,
            snapshot_checksum: None,
        };

        assert_eq!(result.commits_synced, 0);
        assert_eq!(result.issues_synced, 0);
        assert!(!result.has_new_data);
        assert!(result.snapshot_path.is_none());
        assert!(result.snapshot_checksum.is_none());
    }

    #[test]
    fn test_l2_snapshot_result_with_data() {
        let result = L2SnapshotResult {
            snapshot_id: Some(123),
            snapshot_path: Some("/tmp/l2.json".to_string()),
            files_count: 50,
            has_new_data: true,
        };

        assert_eq!(result.snapshot_id, Some(123));
        assert!(result.snapshot_path.is_some());
        assert_eq!(result.files_count, 50);
        assert!(result.has_new_data);
    }

    #[test]
    fn test_l2_snapshot_result_no_data() {
        let result = L2SnapshotResult {
            snapshot_id: None,
            snapshot_path: None,
            files_count: 0,
            has_new_data: false,
        };

        assert!(result.snapshot_id.is_none());
        assert!(result.snapshot_path.is_none());
        assert_eq!(result.files_count, 0);
        assert!(!result.has_new_data);
    }

    #[test]
    fn test_diff_comparison_result_with_changes() {
        let result = DiffComparisonResult {
            report_id: Some(456),
            files_changed: 10,
            has_spec_changes: true,
        };

        assert_eq!(result.report_id, Some(456));
        assert_eq!(result.files_changed, 10);
        assert!(result.has_spec_changes);
    }

    #[test]
    fn test_diff_comparison_result_no_changes() {
        let result = DiffComparisonResult {
            report_id: Some(789),
            files_changed: 0,
            has_spec_changes: false,
        };

        assert_eq!(result.report_id, Some(789));
        assert_eq!(result.files_changed, 0);
        assert!(!result.has_spec_changes);
    }

    #[test]
    fn test_classification_result_with_cves() {
        let result = ClassificationResult {
            classified_count: 15,
            cve_count: 3,
            needs_review_count: 2,
        };

        assert_eq!(result.classified_count, 15);
        assert_eq!(result.cve_count, 3);
        assert_eq!(result.needs_review_count, 2);
    }

    #[test]
    fn test_classification_result_no_cves() {
        let result = ClassificationResult {
            classified_count: 10,
            cve_count: 0,
            needs_review_count: 0,
        };

        assert_eq!(result.classified_count, 10);
        assert_eq!(result.cve_count, 0);
        assert_eq!(result.needs_review_count, 0);
    }

    #[test]
    fn test_report_generation_result() {
        let result = ReportGenerationResult {
            report_id: 999,
            report_status: "success".to_string(),
        };

        assert_eq!(result.report_id, 999);
        assert_eq!(result.report_status, "success");
    }

    #[tokio::test]
    async fn test_stage_l2_snapshot_db_exists() {
        let tracking_model = tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/nonexistent/path".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let snapshot_payload = serde_json::json!({
            "tracking_id": 1,
            "generated_at": Utc::now().to_rfc3339(),
            "origin": "L2",
            "files": [
                {
                    "path": "file1",
                    "size": 10,
                    "sha256": "abc123",
                    "is_binary": false
                }
            ],
            "spec": {
                "path": "specfile",
                "sha256": "def456",
                "version": "1.0",
                "release": "1",
                "content_base64": "Y29udGVudA=="
            },
            "commits": [],
            "issues": []
        });

        let snapshot_model = l2_snapshots::Model {
            id: 1,
            tracking_id: 1,
            snapshot_type: "l2".to_string(),
            payload: snapshot_payload,
            created_at: Utc::now(),
            checksum: "checksum".to_string(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![snapshot_model]])
            .into_connection();

        let executor = PipelineExecutor::new(&db, None);
        let result = executor.stage_l2_snapshot(&tracking_model).await;

        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.snapshot_id, Some(1));
        assert_eq!(res.files_count, 1);
        assert!(res.has_new_data);
    }

    #[tokio::test]
    async fn test_stage_l2_snapshot_repo_exists() {
        use crate::entities::tracking;
        use chrono::Utc;
        use sea_orm::{DatabaseBackend, MockDatabase};
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let l2_repo_path = temp_dir.path().to_str().unwrap().to_string();

        let tracking_model = tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: l2_repo_path.clone(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();

        let executor = PipelineExecutor::new(&db, None);
        let result = executor.stage_l2_snapshot(&tracking_model).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stage_l1_ingestion_tracking_not_active_skips() {
        use crate::entities::tracking;
        use chrono::Utc;
        use sea_orm::{DatabaseBackend, MockDatabase};

        let _guard = EnvVarGuard::set("DEFAULT_PLATFORM", "gitee");

        let tracking_model = tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "gitee".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/tmp".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: None,
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<tracking::Model, _, _>(vec![vec![tracking_model.clone()]])
            .into_connection();

        let executor = PipelineExecutor::new(&db, None);
        let err = executor
            .stage_l1_ingestion(&tracking_model)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("需要 token"));
    }

    #[tokio::test]
    async fn test_stage_diff_comparison_no_diffs() {
        use crate::entities::{compare_reports, l0_commits, l2_snapshots, packages, tracking};
        use chrono::Utc;
        use sea_orm::{DatabaseBackend, MockDatabase};

        let tracking_model = tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let package_model = packages::Model {
            id: 1,
            name: "pkg".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: None,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let now = Utc::now();
        let compare_model = compare_reports::Model {
            id: 42,
            tracking_id: tracking_model.id,
            generated_at: now,
            l2_vs_l1_diff: None,
            l1_vs_l0_diff: None,
            status: "success".to_string(),
            failure_reason: None,
            created_at: now,
            updated_at: now,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<packages::Model, _, _>(vec![vec![package_model]])
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![]])
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![]])
            .append_query_results::<l0_commits::Model, _, _>(vec![vec![]])
            .append_query_results::<compare_reports::Model, _, _>(vec![vec![compare_model]])
            .into_connection();

        let executor = PipelineExecutor::new(&db, None);
        let prev = std::collections::HashMap::new();
        let result = executor
            .stage_diff_comparison(&tracking_model, &prev)
            .await
            .unwrap();

        assert_eq!(result.report_id, Some(42));
        assert_eq!(result.files_changed, 0);
        assert!(!result.has_spec_changes);
    }

    #[tokio::test]
    async fn test_compare_l2_vs_l1_missing_snapshots() {
        let tracking_model = tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let package_model = packages::Model {
            id: 1,
            name: "pkg".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: None,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<packages::Model, _, _>(vec![vec![package_model]])
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![]])
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![]])
            .into_connection();

        let executor = PipelineExecutor::new(&db, None);
        let result = executor.compare_l2_vs_l1(&tracking_model).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_compare_l2_vs_l1_with_snapshots() {
        use crate::entities::l2_snapshots;
        use crate::snapshot::types::{
            CommitEntry, FileEntry, RepositorySnapshot, SnapshotOrigin, SpecEntry,
        };
        use base64::Engine;

        let tracking_model = tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let package_model = packages::Model {
            id: 1,
            name: "pkg".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: None,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let spec_content = r#"
Name: pkg
Version: 1.0.0
Release: 1
Summary: Test package
"#;
        let spec_base64 = base64::engine::general_purpose::STANDARD.encode(spec_content.as_bytes());

        let spec_entry = SpecEntry {
            path: "pkg.spec".to_string(),
            sha256: "spec_hash".to_string(),
            version: Some("1.0.0".to_string()),
            release: Some("1".to_string()),
            content_base64: spec_base64,
        };

        let file_entry = FileEntry {
            path: "file1".to_string(),
            size: 10,
            sha256: "abc123".to_string(),
            is_binary: false,
        };

        let commit_entry = CommitEntry {
            sha: "sha1".to_string(),
            title: "Init".to_string(),
            message: "Initial commit".to_string(),
            author: "dev".to_string(),
            authored_at: Utc::now(),
            url: None,
            stats: crate::snapshot::types::ChangeStats {
                additions: 1,
                deletions: 0,
                files_changed: 1,
            },
            primary_change_type: None,
            cve_list: vec![],
        };

        let l1_snapshot = RepositorySnapshot {
            tracking_id: tracking_model.id,
            generated_at: Utc::now(),
            origin: SnapshotOrigin::L1,
            files: vec![file_entry.clone()],
            spec: Some(spec_entry.clone()),
            commits: vec![commit_entry.clone()],
            issues: vec![],
        };

        let l2_snapshot = RepositorySnapshot {
            tracking_id: tracking_model.id,
            generated_at: Utc::now(),
            origin: SnapshotOrigin::L2,
            files: vec![file_entry],
            spec: Some(spec_entry),
            commits: vec![commit_entry],
            issues: vec![],
        };

        let l1_model = l2_snapshots::Model {
            id: 1,
            tracking_id: tracking_model.id,
            snapshot_type: "l1".to_string(),
            checksum: "c1".to_string(),
            payload: serde_json::to_value(&l1_snapshot).unwrap(),
            created_at: Utc::now(),
        };

        let l2_model = l2_snapshots::Model {
            id: 2,
            tracking_id: tracking_model.id,
            snapshot_type: "l2".to_string(),
            checksum: "c2".to_string(),
            payload: serde_json::to_value(&l2_snapshot).unwrap(),
            created_at: Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<packages::Model, _, _>(vec![vec![package_model]])
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![l1_model]])
            .append_query_results::<l2_snapshots::Model, _, _>(vec![vec![l2_model]])
            .append_query_results::<crate::entities::l2_commit_records::Model, _, _>(vec![vec![]])
            .append_query_results::<crate::entities::l1_commit_records::Model, _, _>(vec![vec![]])
            .into_connection();

        let executor = PipelineExecutor::new(&db, None);
        let result = executor.compare_l2_vs_l1(&tracking_model).await.unwrap();

        assert!(result.is_some());
        let report = result.unwrap();
        assert_eq!(report.package_name, "pkg");
        assert!(report.spec_diff.content_identical);
        assert_eq!(report.patch_diff.l2_added.len(), 0);
        assert_eq!(report.source_diff.l2_added.len(), 0);
    }

    #[tokio::test]
    async fn test_compare_l1_vs_l0_no_l0_info() {
        let tracking_model = tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<l0_commits::Model, _, _>(vec![vec![]])
            .into_connection();

        let executor = PipelineExecutor::new(&db, None);
        let result = executor.compare_l1_vs_l0(&tracking_model).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_l0_version_info_empty() {
        let tracking_model = tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<l0_commits::Model, _, _>(vec![vec![]])
            .into_connection();

        let executor = PipelineExecutor::new(&db, None);
        let result = executor.get_l0_version_info(&tracking_model).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_l1_version_info_unimplemented() {
        let tracking_model = tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let executor = PipelineExecutor::new(&db, None);
        let result = executor.get_l1_version_info(&tracking_model).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_save_comparison_reports_basic() {
        let tracking_model = tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();

        let executor = PipelineExecutor::new(&db, None);

        let l2_report = diff::l2_vs_l1::L2VsL1Report {
            id: None,
            package_name: "pkg".to_string(),
            spec_diff: diff::l2_vs_l1::SpecDiff {
                version_diff: None,
                content_identical: true,
                diff_summary: String::new(),
                key_changes: vec![],
                detailed_comparison: None,
                build_requires_added: vec![],
                build_requires_removed: vec![],
                configure_options_added: vec![],
                configure_options_removed: vec![],
            },
            patch_diff: diff::l2_vs_l1::PatchDiff {
                l1_total: 0,
                l2_total: 0,
                l2_added: vec![],
                l2_modified: vec![],
                l2_removed: vec![],
                identical: vec![],
            },
            source_diff: diff::l2_vs_l1::SourceDiff {
                l1_total: 0,
                l2_total: 0,
                l2_added: vec![],
                l2_removed: vec![],
                l2_modified: vec![],
            },
            customization_analysis: diff::l2_vs_l1::CustomizationAnalysis {
                total_customizations: 0,
                by_type: std::collections::HashMap::new(),
                summary: String::new(),
            },
            sync_recommendations: vec![],
            conflicts: vec![],
            commit_diff: diff::l2_vs_l1::CommitDiff {
                l1_commits_count: 0,
                l2_commits_count: 0,
                behind_commits: vec![],
                base_commit: None,
                base_version_release: None,
            },
            created_at: Utc::now(),
        };

        let l1_report = diff::l1_vs_l0::L1VsL0Report {
            id: None,
            package_name: "pkg".to_string(),
            current_version: "1.0.0".to_string(),
            latest_stable: "1.0.0".to_string(),
            latest_version: "1.0.0".to_string(),
            version_behind: 0,
            upgradable_versions: vec![],
            patch_analysis: diff::l1_vs_l0::PatchAnalysis {
                total_patches: 0,
                merged_in_upstream: vec![],
                still_needed: vec![],
                can_be_removed_after_upgrade: 0,
            },
            cve_analysis: diff::l1_vs_l0::CveAnalysis {
                total_cves: 0,
                fixed_in_upstream: vec![],
                not_fixed_in_upstream: vec![],
            },
            recommendations: vec![],
            created_at: Utc::now(),
        };

        let l2_opt = Some(l2_report);
        let l1_opt = Some(l1_report);

        let result = executor
            .save_comparison_reports(&tracking_model, &l2_opt, &l1_opt)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stage_backport_suggestion_uses_advisor() {
        let tracking_model = tracking::Model {
            id: 1,
            package_id: 10,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results(vec![Vec::<crate::entities::packages::Model>::new()])
            .into_connection();

        let executor = PipelineExecutor::new(&db, None);
        let prev = std::collections::HashMap::new();
        let result = executor
            .stage_backport_suggestion(&tracking_model, &prev)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stage_classification_empty_pending() {
        let tracking_model = tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<crate::entities::l1_commit_records::Model, _, _>(vec![vec![]])
            .into_connection();

        let executor = PipelineExecutor::new(&db, None);
        let prev = std::collections::HashMap::new();
        let result = executor
            .stage_classification(&tracking_model, &prev)
            .await
            .unwrap();
        assert_eq!(result.classified_count, 0);
        assert_eq!(result.cve_count, 0);
        assert_eq!(result.needs_review_count, 0);
    }

    #[tokio::test]
    async fn test_stage_classification_with_pending_commits() {
        use crate::analyzer::{ChangeClassification, ChangeType};
        use crate::entities::l1_commit_records::{ActiveModel as L1ActiveModel, Model as L1Model};
        use chrono::Utc;
        use sea_orm::MockExecResult;

        let tracking_model = tracking::Model {
            id: 1,
            package_id: 1,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let pending_commit = L1Model {
            id: 1,
            tracking_id: tracking_model.id,
            commit_sha: "sha1".to_string(),
            commit_message: "Fix CVE-2024-0001".to_string(),
            author_name: "dev".to_string(),
            author_email: "dev@example.com".to_string(),
            committed_at: Utc::now(),
            created_at: Utc::now(),
            change_type: None,
            primary_change_type: None,
            cve_list: None,
            spec_changed: false,
            patch_stats: None,
            classification_status: "pending".to_string(),
            classification_notes: None,
            sync_status: "pending".to_string(),
            synced_to_l2_commit: None,
            synced_at: None,
            api_url: "http://example.com/commit/sha1".to_string(),
            fetched_at: Utc::now(),
            files_changed_count: 1,
            additions: 10,
            deletions: 2,
            updated_at: Utc::now(),
            spec_version: None,
            spec_release: None,
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<L1Model, _, _>(vec![vec![pending_commit.clone()]])
            .append_query_results::<L1Model, _, _>(vec![vec![pending_commit.clone()]])
            .append_exec_results(vec![MockExecResult {
                last_insert_id: 1,
                rows_affected: 1,
            }])
            .into_connection();

        let executor = PipelineExecutor::new(&db, None);
        let prev = std::collections::HashMap::new();

        let classification = ChangeClassification {
            primary_type: ChangeType::CVE,
            cve_numbers: vec!["CVE-2024-0001".to_string()],
            ..Default::default()
        };

        let mut active_commit: L1ActiveModel = pending_commit.into();
        active_commit.primary_change_type =
            Set(Some(classification.primary_type.as_str().to_string()));
        active_commit.cve_list = Set(Some(
            serde_json::to_value(&classification.cve_numbers).unwrap(),
        ));
        active_commit.spec_changed = Set(classification.has_spec_change);
        active_commit.classification_status = Set("done".to_string());
        active_commit.updated_at = Set(Utc::now());
        let _ = active_commit.update(&db).await.unwrap();

        let result = executor
            .stage_classification(&tracking_model, &prev)
            .await
            .unwrap();

        assert_eq!(result.classified_count, 0);
        assert_eq!(result.cve_count, 0);
        assert_eq!(result.needs_review_count, 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_stage_report_generation_min() {
        use crate::entities::{
            compare_reports, l1_commit_records, l2_snapshots, packages, tracking, tracking_reports,
        };
        use chrono::Utc;
        use sea_orm::{DatabaseBackend, MockDatabase};

        let _risk_enabled_guard = EnvVarGuard::set("RISK_CREATE_ENABLED", "false");

        let tracking_model = tracking::Model {
            id: 2,
            package_id: 3,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let package_model = packages::Model {
            id: 3,
            name: "pkg".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: None,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let l2_vs_l1_diff = serde_json::json!({
            "commit_diff": {
                "base_version_release": ["1.0", "1"],
                "behind_commits": [
                    {"sha": "sha-001"}
                ]
            }
        });

        let compare_model = compare_reports::Model {
            id: 42,
            tracking_id: tracking_model.id,
            generated_at: Utc::now(),
            l2_vs_l1_diff: Some(l2_vs_l1_diff),
            l1_vs_l0_diff: None,
            status: "ok".to_string(),
            failure_reason: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let commit_model = l1_commit_records::Model {
            id: 7,
            tracking_id: tracking_model.id,
            commit_sha: "sha-001".to_string(),
            commit_message: "Fix bug".to_string(),
            author_name: "dev".to_string(),
            author_email: "dev@example.com".to_string(),
            committed_at: Utc::now(),
            created_at: Utc::now(),
            change_type: None,
            primary_change_type: Some("Bugfix".to_string()),
            cve_list: None,
            spec_changed: false,
            patch_stats: None,
            classification_status: "pending".to_string(),
            classification_notes: None,
            sync_status: "pending".to_string(),
            synced_to_l2_commit: None,
            synced_at: None,
            api_url: "http://example.com/commit/sha-001".to_string(),
            fetched_at: Utc::now(),
            files_changed_count: 1,
            additions: 10,
            deletions: 2,
            updated_at: Utc::now(),
            spec_version: None,
            spec_release: None,
        };

        let inserted_report = tracking_reports::Model {
            id: 99,
            tracking_id: tracking_model.id,
            generated_at: Utc::now(),
            diff_summary: serde_json::json!({}),
            representative_changes: None,
            source: "pipeline".to_string(),
            status: "success".to_string(),
            failure_reason: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<packages::Model, _, _>(vec![vec![package_model.clone()]])
            .append_query_results::<compare_reports::Model, _, _>(vec![vec![compare_model.clone()]])
            .append_query_results::<l1_commit_records::Model, _, _>(vec![
                vec![commit_model.clone()],
            ])
            .append_query_results::<tracking_reports::Model, _, _>(vec![vec![
                inserted_report.clone()
            ]])
            .into_connection();

        let executor = PipelineExecutor::new(&db, None);
        let mut prev = std::collections::HashMap::new();
        let diff_details = serde_json::json!({"report_id": 42});
        let stage = StageResult::success(
            PipelineStage::DiffComparison,
            "ok".to_string(),
            Utc::now(),
            diff_details,
        );
        prev.insert(PipelineStage::DiffComparison, stage);

        let result = executor
            .stage_report_generation(&tracking_model, &prev)
            .await
            .unwrap();
        assert_eq!(result.report_status, "success".to_string());
        assert!(result.report_id > 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_stage_report_generation_calls_risk_create() {
        use crate::entities::{
            compare_reports, l1_commit_records, l2_snapshots, packages, tracking, tracking_reports,
        };
        use chrono::{TimeZone, Utc};
        use httpmock::prelude::*;
        use sea_orm::{DatabaseBackend, MockDatabase};

        let server = MockServer::start();
        let risk_create_url = format!("{}/risk/create", server.base_url());
        let _risk_url_guard = EnvVarGuard::set("RISK_CREATE_URL", &risk_create_url);
        let _risk_enabled_guard = EnvVarGuard::set("RISK_CREATE_ENABLED", "true");
        let _risk_timeout_guard = EnvVarGuard::set("RISK_HTTP_TIMEOUT_SECS", "2");

        let fixed_time = Utc.timestamp_opt(1_700_000_000, 0).unwrap();

        let expected_body = serde_json::json!({
            "description": "Fix bug\nhttp://example.com/commit/sha-001",
            "level": 2,
            "reporter": "track-system",
            "type": "Bugfix",
            "software": "pkg",
            "version": "1.0",
            "release": "1",
            "platform": "noarch",
            "disclosure_time": fixed_time.to_rfc3339(),
            "source": "owner",
            "package_id": 0,
            "inner_secret": "Ctyun@123"
        });

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/risk/create")
                .json_body(expected_body);
            then.status(200).json_body(serde_json::json!({"ok": true}));
        });

        let tracking_model = tracking::Model {
            id: 2,
            package_id: 3,
            distro_id: 1,
            l1_branch: "main".to_string(),
            l1_repo_owner: "owner".to_string(),
            l1_repo_name: "repo".to_string(),
            l2_branch: "local".to_string(),
            l2_repo_path: "/path".to_string(),
            tracking_status: "idle".to_string(),
            last_sync_time: Some(Utc::now()),
            last_l1_commit_sha: None,
            last_l2_commit_sha: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        let package_model = packages::Model {
            id: 3,
            name: "pkg".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: None,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let l2_vs_l1_diff = serde_json::json!({
            "commit_diff": {
                "base_version_release": ["1.0", "1"],
                "behind_commits": [
                    {"sha": "sha-001"}
                ]
            }
        });

        let compare_model = compare_reports::Model {
            id: 42,
            tracking_id: tracking_model.id,
            generated_at: Utc::now(),
            l2_vs_l1_diff: Some(l2_vs_l1_diff),
            l1_vs_l0_diff: None,
            status: "ok".to_string(),
            failure_reason: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let commit_model = l1_commit_records::Model {
            id: 7,
            tracking_id: tracking_model.id,
            commit_sha: "sha-001".to_string(),
            commit_message: "Fix bug".to_string(),
            author_name: "dev".to_string(),
            author_email: "dev@example.com".to_string(),
            committed_at: fixed_time,
            created_at: Utc::now(),
            change_type: None,
            primary_change_type: Some("Bugfix".to_string()),
            cve_list: None,
            spec_changed: false,
            patch_stats: None,
            classification_status: "pending".to_string(),
            classification_notes: None,
            sync_status: "pending".to_string(),
            synced_to_l2_commit: None,
            synced_at: None,
            api_url: "http://example.com/commit/sha-001".to_string(),
            fetched_at: Utc::now(),
            files_changed_count: 1,
            additions: 10,
            deletions: 2,
            updated_at: Utc::now(),
            spec_version: None,
            spec_release: None,
        };

        let inserted_report = tracking_reports::Model {
            id: 99,
            tracking_id: tracking_model.id,
            generated_at: Utc::now(),
            diff_summary: serde_json::json!({}),
            representative_changes: None,
            source: "pipeline".to_string(),
            status: "success".to_string(),
            failure_reason: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<packages::Model, _, _>(vec![vec![package_model.clone()]])
            .append_query_results::<compare_reports::Model, _, _>(vec![vec![compare_model.clone()]])
            .append_query_results::<l1_commit_records::Model, _, _>(vec![
                vec![commit_model.clone()],
            ])
            .append_query_results::<tracking_reports::Model, _, _>(vec![vec![
                inserted_report.clone()
            ]])
            .into_connection();

        let executor = PipelineExecutor::new(&db, None);
        let mut prev = std::collections::HashMap::new();
        let diff_details = serde_json::json!({"report_id": 42});
        let stage = StageResult::success(
            PipelineStage::DiffComparison,
            "ok".to_string(),
            Utc::now(),
            diff_details,
        );
        prev.insert(PipelineStage::DiffComparison, stage);

        let result = executor
            .stage_report_generation(&tracking_model, &prev)
            .await
            .unwrap();
        assert_eq!(result.report_status, "success".to_string());
        assert!(result.report_id > 0);
        mock.assert_calls(1);
    }
}
