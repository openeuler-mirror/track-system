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
