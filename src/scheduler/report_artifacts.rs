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

//! Pipeline report artifact generation.

use anyhow::{Context, Result};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    cmp::Ordering,
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tracing::{info, warn};

const XLSX_TEMPLATE_NAME: &str = "CVE/issue漏洞修复.xlsx";
const SHEET_HEADERS: [&str; 7] = [
    "软件包",
    "CVE/ISSUE编号",
    "上游修复版本",
    "CTyunOS当前版本",
    "系统版本",
    "描述",
    "星空提单号",
];
const ISSUE_START_NUMBER: u32 = 10200;
const ISSUE_MAX_NUMBER: u32 = 9_999_999;
const ISSUE_RESERVATION_BLOCK_SIZE: u32 = 10_000;
const ISSUE_NUMBER_EPOCH_UNIX_SECS: i64 = 1_767_225_600; // 2026-01-01T00:00:00Z
const ISSUE_NUMBER_STATE_FILE: &str = ".track-system-cve-issue-number";
const DEFAULT_XLSX_MAX_PACKAGES: usize = 30;
const DEFAULT_SYSTEM_VERSION_BLACKLIST: [&str; 2] = ["CTyunOS2.0.1", "CTyunOS25.05"];

#[derive(Debug, Clone, Serialize)]
pub struct ReportArtifact {
    pub artifact_type: String,
    pub path: String,
    pub format: String,
    pub rows: usize,
    pub source: String,
    pub template: String,
    pub generated_at: String,
    #[serde(skip_serializing)]
    pub preview_rows: Vec<CveFixComparisonRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CveFixComparisonRow {
    pub package: String,
    pub cve_or_issue: String,
    pub upstream_fixed_version: String,
    pub ctyunos_current_version: String,
    pub system_version: String,
    pub description: String,
    pub xingkong_ticket_no: String,
    pub commit_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CveFixComparisonInput {
    pub tracking_id: i32,
    pub package_name: String,
    pub system_version: String,
    pub ctyunos_current_version: String,
    pub default_upstream_version: String,
    pub commit_reports: Vec<Value>,
}

#[derive(Debug, Clone)]
pub struct RoundCveFixComparisonWriter {
    output_path_prefix: PathBuf,
    issue_start_number: u32,
    inputs: Vec<CveFixComparisonInput>,
    artifacts: Vec<ReportArtifact>,
}

impl RoundCveFixComparisonWriter {
    pub fn new() -> Self {
        Self {
            output_path_prefix: cve_fix_comparison_round_output_path_prefix(),
            issue_start_number: allocate_issue_start_number(),
            inputs: Vec::new(),
            artifacts: Vec::new(),
        }
    }

    pub fn append_input(&mut self, input: CveFixComparisonInput) -> Result<ReportArtifact> {
        self.inputs.push(input);
        self.artifacts = self.rewrite()?;
        self.artifacts
            .last()
            .cloned()
            .context("生成调度轮次 CVE 漏洞修复对比 xlsx 失败：没有 artifact")
    }

    pub fn artifacts(&self) -> &[ReportArtifact] {
        &self.artifacts
    }

    fn rewrite(&self) -> Result<Vec<ReportArtifact>> {
        let row_volumes = cve_fix_comparison_row_volumes_for_inputs_with_issue_start(
            &self.inputs,
            self.issue_start_number,
        );
        let mut artifacts = Vec::new();

        for (idx, rows) in row_volumes.iter().enumerate() {
            let output_path = cve_fix_comparison_round_volume_output_path(
                &self.output_path_prefix,
                idx + 1,
                row_volumes.len(),
            );
            write_cve_fix_comparison_xlsx(&output_path, rows)?;
            artifacts.push(round_artifact_for_path(&output_path, rows));
        }

        Ok(artifacts)
    }
}

pub fn create_cve_fix_comparison_artifact(
    tracking_id: i32,
    package_name: &str,
    system_version: &str,
    ctyunos_current_version: &str,
    default_upstream_version: &str,
    commit_reports: &[Value],
) -> Result<ReportArtifact> {
    let rows = cve_fix_comparison_rows_with_issue_start(
        package_name,
        system_version,
        ctyunos_current_version,
        default_upstream_version,
        commit_reports,
        allocate_issue_start_number(),
    );
    let output_path = cve_fix_comparison_output_path(tracking_id, package_name);
    write_cve_fix_comparison_xlsx(&output_path, &rows)?;

    Ok(ReportArtifact {
        artifact_type: "cve_fix_comparison_xlsx".to_string(),
        path: output_path.to_string_lossy().to_string(),
        format: "xlsx".to_string(),
        rows: rows.len(),
        source: "pipeline".to_string(),
        template: XLSX_TEMPLATE_NAME.to_string(),
        generated_at: Utc::now().to_rfc3339(),
        preview_rows: rows,
    })
}

pub fn create_round_cve_fix_comparison_artifact(
    inputs: &[CveFixComparisonInput],
) -> Result<ReportArtifact> {
    let rows =
        cve_fix_comparison_rows_for_inputs_with_issue_start(inputs, allocate_issue_start_number());
    let output_path = cve_fix_comparison_round_output_path_prefix().with_extension("xlsx");
    write_cve_fix_comparison_xlsx(&output_path, &rows)?;
    Ok(round_artifact_for_path(&output_path, &rows))
}

fn cve_fix_comparison_output_path(tracking_id: i32, package_name: &str) -> PathBuf {
    let base_dir = report_artifact_base_dir();
    let timestamp = Utc::now().format("%Y%m%d%H%M%S").to_string();
    let package = sanitize_filename(package_name);
    base_dir.join(format!(
        "CVE-ISSUE修复列表_{}_tracking_{}_{}.xlsx",
        package, tracking_id, timestamp
    ))
}

fn cve_fix_comparison_round_output_path_prefix() -> PathBuf {
    let base_dir = report_artifact_base_dir();
    let timestamp = Utc::now().format("%Y%m%d%H%M%S%3f").to_string();
    base_dir.join(format!("CVE-ISSUE修复列表{}", timestamp))
}

fn cve_fix_comparison_round_volume_output_path(
    prefix: &Path,
    volume_index: usize,
    volume_count: usize,
) -> PathBuf {
    if volume_count <= 1 {
        return prefix.with_extension("xlsx");
    }

    let file_name = prefix
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("CVE-ISSUE修复列表");
    let parent = prefix.parent().unwrap_or_else(|| Path::new(""));
    parent.join(format!("{file_name}_part{volume_index:02}.xlsx"))
}

fn report_artifact_base_dir() -> PathBuf {
    std::env::var("TRACK_REPORT_ARTIFACT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("track-system").join("reports"))
}

fn allocate_issue_start_number() -> u32 {
    match reserve_issue_number_block() {
        Ok(number) => number,
        Err(error) => {
            let fallback = time_based_issue_start_number(Utc::now().timestamp());
            warn!(
                error = %error,
                fallback,
                "分配持久化 ISSUE 号段失败，使用时间号段兜底"
            );
            fallback
        }
    }
}

fn reserve_issue_number_block() -> Result<u32> {
    let state_path = issue_number_state_path();
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建 ISSUE 号状态目录失败: {}", parent.display()))?;
    }

    let time_candidate = time_based_issue_start_number(Utc::now().timestamp());
    let previous = fs::read_to_string(&state_path)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok());
    let reserved = previous
        .map(|previous| previous.saturating_add(ISSUE_RESERVATION_BLOCK_SIZE))
        .unwrap_or(time_candidate)
        .max(time_candidate)
        .clamp(ISSUE_START_NUMBER, ISSUE_MAX_NUMBER);

    fs::write(&state_path, format!("{reserved}\n"))
        .with_context(|| format!("写入 ISSUE 号状态文件失败: {}", state_path.display()))?;
    Ok(reserved)
}

fn issue_number_state_path() -> PathBuf {
    std::env::var("TRACK_XLSX_ISSUE_NUMBER_STATE_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| report_artifact_base_dir().join(ISSUE_NUMBER_STATE_FILE))
}

fn time_based_issue_start_number(unix_secs: i64) -> u32 {
    let elapsed_minutes = unix_secs.saturating_sub(ISSUE_NUMBER_EPOCH_UNIX_SECS) / 60;
    let candidate = ISSUE_START_NUMBER as i64 + elapsed_minutes;
    candidate.clamp(ISSUE_START_NUMBER as i64, ISSUE_MAX_NUMBER as i64) as u32
}

fn round_artifact_for_path(path: &Path, rows: &[CveFixComparisonRow]) -> ReportArtifact {
    ReportArtifact {
        artifact_type: "cve_fix_comparison_xlsx".to_string(),
        path: path.to_string_lossy().to_string(),
        format: "xlsx".to_string(),
        rows: rows.len(),
        source: "scheduler_round".to_string(),
        template: XLSX_TEMPLATE_NAME.to_string(),
        generated_at: Utc::now().to_rfc3339(),
        preview_rows: rows.to_vec(),
    }
}

#[allow(unused)]
fn cve_fix_comparison_rows(
    package_name: &str,
    system_version: &str,
    ctyunos_current_version: &str,
    default_upstream_version: &str,
    commit_reports: &[Value],
) -> Vec<CveFixComparisonRow> {
    cve_fix_comparison_rows_with_issue_start(
        package_name,
        system_version,
        ctyunos_current_version,
        default_upstream_version,
        commit_reports,
        allocate_issue_start_number(),
    )
}

fn cve_fix_comparison_rows_with_issue_start(
    package_name: &str,
    system_version: &str,
    ctyunos_current_version: &str,
    default_upstream_version: &str,
    commit_reports: &[Value],
    issue_start_number: u32,
) -> Vec<CveFixComparisonRow> {
    let input = CveFixComparisonInput {
        tracking_id: 0,
        package_name: package_name.to_string(),
        system_version: system_version.to_string(),
        ctyunos_current_version: ctyunos_current_version.to_string(),
        default_upstream_version: default_upstream_version.to_string(),
        commit_reports: commit_reports.to_vec(),
    };
    cve_fix_comparison_rows_for_inputs_with_issue_start(&[input], issue_start_number)
}

pub fn cve_fix_comparison_rows_for_inputs(
    inputs: &[CveFixComparisonInput],
) -> Vec<CveFixComparisonRow> {
    cve_fix_comparison_rows_for_inputs_with_issue_start(inputs, allocate_issue_start_number())
}

fn cve_fix_comparison_rows_for_inputs_with_issue_start(
    inputs: &[CveFixComparisonInput],
    issue_start_number: u32,
) -> Vec<CveFixComparisonRow> {
    cve_fix_comparison_row_volumes_for_inputs_with_issue_start(inputs, issue_start_number)
        .into_iter()
        .flatten()
        .collect()
}

fn cve_fix_comparison_row_volumes_for_inputs_with_issue_start(
    inputs: &[CveFixComparisonInput],
    issue_start_number: u32,
) -> Vec<Vec<CveFixComparisonRow>> {
    let ticket_no =
        std::env::var("TRACK_XINGKONG_TICKET_NO").unwrap_or_else(|_| "97883".to_string());
    let mut issue_generator = IssueNumberGenerator::new(issue_start_number);
    let mut volumes: Vec<Vec<RowGroup>> = vec![Vec::new()];
    let max_packages = xlsx_max_packages();

    for input in inputs {
        collect_rows_into_volumes(
            &input.package_name,
            &input.system_version,
            &input.ctyunos_current_version,
            &input.default_upstream_version,
            &input.commit_reports,
            &ticket_no,
            &mut issue_generator,
            &mut volumes,
            max_packages,
        );
    }

    let mut rows = volumes
        .into_iter()
        .map(row_groups_to_rows)
        .filter(|rows| !rows.is_empty())
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(Vec::new());
    }
    rows
}

fn collect_rows_into_volumes(
    package_name: &str,
    system_version: &str,
    ctyunos_current_version: &str,
    default_upstream_version: &str,
    commit_reports: &[Value],
    ticket_no: &str,
    issue_generator: &mut IssueNumberGenerator,
    volumes: &mut Vec<Vec<RowGroup>>,
    max_packages: usize,
) {
    let system_version = normalize_system_version(system_version);
    if system_version_is_blacklisted(&system_version) {
        info!(
            package = package_name,
            system_version = %system_version,
            commit_reports = commit_reports.len(),
            "跳过 CVE 漏洞修复对比 xlsx 数据：系统版本命中黑名单"
        );
        return;
    }

    for commit in commit_reports {
        let description = text_field(commit, "Description");
        let commit_url = text_field(commit, "Url");
        let mut identifiers = cve_list_field(commit, "CVEList");
        identifiers.extend(extract_cve_identifiers(&description));
        identifiers.sort();
        identifiers.dedup();

        if identifiers.is_empty() {
            identifiers.push(issue_generator.next_issue());
        }

        let upstream_fixed_version = upstream_fixed_version(commit, default_upstream_version);
        let description_entry =
            description_entry_for_commit(&description, &commit_url, identifiers.as_slice());
        let volume_index = volume_index_for_package(volumes, package_name, max_packages);
        let groups = volumes
            .get_mut(volume_index)
            .expect("volume index should exist");

        let group = groups.iter_mut().find(|group| {
            group.package == package_name
                && group.ctyunos_current_version == ctyunos_current_version
                && group.system_version == system_version
        });

        let group = match group {
            Some(group) => group,
            None => {
                groups.push(RowGroup {
                    package: package_name.to_string(),
                    upstream_fixed_version: upstream_fixed_version.clone(),
                    ctyunos_current_version: ctyunos_current_version.to_string(),
                    system_version: system_version.clone(),
                    identifiers: Vec::new(),
                    descriptions: Vec::new(),
                    xingkong_ticket_no: ticket_no.to_string(),
                    commit_url: None,
                });
                groups.last_mut().expect("just pushed row group")
            }
        };

        if is_newer_version_release(&upstream_fixed_version, &group.upstream_fixed_version) {
            group.upstream_fixed_version = upstream_fixed_version;
        }

        for identifier in identifiers {
            if !group.identifiers.contains(&identifier) {
                group.identifiers.push(identifier);
            }
        }
        if !description_entry.is_empty() && !group.descriptions.contains(&description_entry) {
            group.descriptions.push(description_entry);
        }
        if group.commit_url.is_none() && !commit_url.is_empty() {
            group.commit_url = Some(commit_url);
        }
    }
}

fn volume_index_for_package(
    volumes: &mut Vec<Vec<RowGroup>>,
    package_name: &str,
    max_packages: usize,
) -> usize {
    if let Some(idx) = volumes
        .iter()
        .position(|groups| groups.iter().any(|group| group.package == package_name))
    {
        return idx;
    }

    if max_packages == 0 {
        return 0;
    }

    let last_idx = volumes.len().saturating_sub(1);
    if unique_package_count(&volumes[last_idx]) >= max_packages {
        volumes.push(Vec::new());
        volumes.len() - 1
    } else {
        last_idx
    }
}

fn xlsx_max_packages() -> usize {
    std::env::var("TRACK_XLSX_MAX_PACKAGES")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(DEFAULT_XLSX_MAX_PACKAGES)
}

fn unique_package_count(groups: &[RowGroup]) -> usize {
    groups
        .iter()
        .map(|group| group.package.as_str())
        .collect::<HashSet<_>>()
        .len()
}

fn system_version_is_blacklisted(system_version: &str) -> bool {
    system_version_blacklist()
        .iter()
        .any(|blacklisted| blacklisted.eq_ignore_ascii_case(system_version.trim()))
}

fn system_version_blacklist() -> Vec<String> {
    match std::env::var("TRACK_XLSX_SYSTEM_VERSION_BLACKLIST") {
        Ok(value) => value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(normalize_system_version)
            .collect(),
        Err(_) => DEFAULT_SYSTEM_VERSION_BLACKLIST
            .iter()
            .map(|version| version.to_string())
            .collect(),
    }
