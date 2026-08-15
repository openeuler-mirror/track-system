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
}

fn row_groups_to_rows(groups: Vec<RowGroup>) -> Vec<CveFixComparisonRow> {
    groups
        .into_iter()
        .map(|group| CveFixComparisonRow {
            package: group.package,
            cve_or_issue: group.identifiers.join(","),
            upstream_fixed_version: group.upstream_fixed_version,
            ctyunos_current_version: group.ctyunos_current_version,
            system_version: group.system_version,
            description: group.descriptions.join("\n"),
            xingkong_ticket_no: group.xingkong_ticket_no,
            commit_url: group.commit_url,
        })
        .collect()
}

struct IssueNumberGenerator {
    next: u32,
}

struct RowGroup {
    package: String,
    upstream_fixed_version: String,
    ctyunos_current_version: String,
    system_version: String,
    identifiers: Vec<String>,
    descriptions: Vec<String>,
    xingkong_ticket_no: String,
    commit_url: Option<String>,
}

impl IssueNumberGenerator {
    fn new(start: u32) -> Self {
        Self { next: start }
    }

    fn next_issue(&mut self) -> String {
        let issue = format!("ISSUE-{}", self.next);
        self.next += 1;
        issue
    }
}

fn write_cve_fix_comparison_xlsx(path: &Path, rows: &[CveFixComparisonRow]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建报告附件目录失败: {}", parent.display()))?;
    }

    let sheets = sheet_rows_by_system_version(rows);
    let mut files = vec![
        (
            "[Content_Types].xml".to_string(),
            content_types_xml(sheets.len()).into_bytes(),
        ),
        ("_rels/.rels".to_string(), root_rels_xml().into_bytes()),
        ("docProps/app.xml".to_string(), app_xml().into_bytes()),
        ("docProps/core.xml".to_string(), core_xml().into_bytes()),
        (
            "xl/workbook.xml".to_string(),
            workbook_xml(&sheets).into_bytes(),
        ),
        (
            "xl/_rels/workbook.xml.rels".to_string(),
            workbook_rels_xml(sheets.len()).into_bytes(),
        ),
        ("xl/styles.xml".to_string(), styles_xml().into_bytes()),
    ];

    for (idx, sheet) in sheets.iter().enumerate() {
        let sheet_id = idx + 1;
        files.push((
            format!("xl/worksheets/sheet{sheet_id}.xml"),
            sheet_xml(&sheet.rows).into_bytes(),
        ));
        files.push((
            format!("xl/worksheets/_rels/sheet{sheet_id}.xml.rels"),
            sheet_rels_xml(&sheet.rows).into_bytes(),
        ));
    }

    write_stored_zip(path, &files)
        .with_context(|| format!("写入 xlsx 报告附件失败: {}", path.display()))
}

#[derive(Debug, Clone)]
struct SheetRows {
    name: String,
    rows: Vec<CveFixComparisonRow>,
}

fn sheet_rows_by_system_version(rows: &[CveFixComparisonRow]) -> Vec<SheetRows> {
    let mut sheets: Vec<SheetRows> = Vec::new();
    for row in rows {
        let sheet_name = sanitize_sheet_name(&row.system_version);
        if let Some(sheet) = sheets.iter_mut().find(|sheet| sheet.name == sheet_name) {
            sheet.rows.push(row.clone());
        } else {
            sheets.push(SheetRows {
                name: sheet_name,
                rows: vec![row.clone()],
            });
        }
    }

    if sheets.is_empty() {
        sheets.push(SheetRows {
            name: "NoData".to_string(),
            rows: Vec::new(),
        });
    }

    dedupe_sheet_names(&mut sheets);
    sheets
}

fn sanitize_sheet_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|ch| match ch {
            ':' | '\\' | '/' | '?' | '*' | '[' | ']' => '_',
            _ => ch,
        })
        .collect::<String>()
        .trim()
        .to_string();
    let name = if sanitized.is_empty() {
        "Sheet".to_string()
    } else {
        sanitized
    };
    name.chars().take(31).collect()
}

fn dedupe_sheet_names(sheets: &mut [SheetRows]) {
    let mut seen = HashSet::new();
    for sheet in sheets {
        if seen.insert(sheet.name.clone()) {
            continue;
        }

        let base = sheet.name.chars().take(28).collect::<String>();
        let mut idx = 2;
        loop {
            let candidate = format!("{base}_{idx}");
            if seen.insert(candidate.clone()) {
                sheet.name = candidate;
                break;
            }
            idx += 1;
        }
    }
}

fn content_types_xml(sheet_count: usize) -> String {
    let mut xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
"#
    .to_string();
    for sheet_id in 1..=sheet_count {
        xml.push_str(&format!(
            r#"  <Override PartName="/xl/worksheets/sheet{sheet_id}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
"#
        ));
    }
    xml.push_str(
        r#"  <Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
</Types>"#,
    );
    xml
}

fn root_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#
        .to_string()
}

fn app_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
  <Application>track-system</Application>
  <DocSecurity>0</DocSecurity>
  <ScaleCrop>false</ScaleCrop>
  <HeadingPairs><vt:vector size="2" baseType="variant"><vt:variant><vt:lpstr>Worksheets</vt:lpstr></vt:variant><vt:variant><vt:i4>1</vt:i4></vt:variant></vt:vector></HeadingPairs>
  <TitlesOfParts><vt:vector size="1" baseType="lpstr"><vt:lpstr>Sheet1</vt:lpstr></vt:vector></TitlesOfParts>
</Properties>"#
        .to_string()
}

fn core_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <dc:creator>track-system</dc:creator>
  <cp:lastModifiedBy>track-system</cp:lastModifiedBy>
  <dcterms:created xsi:type="dcterms:W3CDTF">{now}</dcterms:created>
  <dcterms:modified xsi:type="dcterms:W3CDTF">{now}</dcterms:modified>
</cp:coreProperties>"#,
        now = Utc::now().to_rfc3339()
    )
}

fn workbook_xml(sheets: &[SheetRows]) -> String {
    let mut xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>"#
        .to_string();
    for (idx, sheet) in sheets.iter().enumerate() {
        let sheet_id = idx + 1;
        xml.push_str(&format!(
            r#"<sheet name="{}" sheetId="{sheet_id}" r:id="rId{sheet_id}"/>"#,
            xml_escape(&sheet.name)
        ));
    }
    xml.push_str("</sheets></workbook>");
    xml
}

fn workbook_rels_xml(sheet_count: usize) -> String {
    let mut xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
"#
    .to_string();
    for sheet_id in 1..=sheet_count {
        xml.push_str(&format!(
            r#"  <Relationship Id="rId{sheet_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{sheet_id}.xml"/>
"#
        ));
    }
    xml.push_str(&format!(
        r#"  <Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#,
        sheet_count + 1
    ));
    xml
}

fn styles_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <fonts count="3">
    <font><sz val="11"/><name val="宋体"/></font>
    <font><b/><sz val="11"/><name val="宋体"/></font>
    <font><u/><sz val="11"/><color rgb="FF0563C1"/><name val="宋体"/></font>
  </fonts>
  <fills count="2">
    <fill><patternFill patternType="none"/></fill>
    <fill><patternFill patternType="gray125"/></fill>
  </fills>
  <borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders>
  <cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>
  <cellXfs count="4">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>
    <xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1" applyAlignment="1"><alignment horizontal="center" vertical="center" wrapText="1"/></xf>
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0" applyAlignment="1"><alignment vertical="top" wrapText="1"/></xf>
    <xf numFmtId="0" fontId="2" fillId="0" borderId="0" xfId="0" applyFont="1" applyAlignment="1"><alignment vertical="top" wrapText="1"/></xf>
  </cellXfs>
  <cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>
</styleSheet>"#
        .to_string()
}

fn sheet_xml(rows: &[CveFixComparisonRow]) -> String {
    let row_count = rows.len() + 1;
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="A1:G{row_count}"/>
  <sheetViews><sheetView workbookViewId="0"/></sheetViews>
  <sheetFormatPr defaultRowHeight="16"/>
  <cols>
    <col min="1" max="1" width="28" customWidth="1"/>
    <col min="2" max="2" width="48" customWidth="1"/>
    <col min="3" max="3" width="22" customWidth="1"/>
    <col min="4" max="4" width="24" customWidth="1"/>
    <col min="5" max="5" width="24" customWidth="1"/>
    <col min="6" max="6" width="76" customWidth="1"/>
    <col min="7" max="7" width="18" customWidth="1"/>
  </cols>
  <sheetData>"#
    );

    xml.push_str(r#"<row r="1" ht="22">"#);
    for (idx, header) in SHEET_HEADERS.iter().enumerate() {
        xml.push_str(&inline_str_cell(&cell_ref(idx, 1), header, 1));
    }
    xml.push_str("</row>");

    for (row_idx, row) in rows.iter().enumerate() {
        let excel_row = row_idx + 2;
        xml.push_str(&format!(r#"<row r="{excel_row}" ht="46">"#));
        let values = [
            row.package.as_str(),
            row.cve_or_issue.as_str(),
            row.upstream_fixed_version.as_str(),
            row.ctyunos_current_version.as_str(),
            row.system_version.as_str(),
            row.description.as_str(),
            row.xingkong_ticket_no.as_str(),
        ];
        for (col_idx, value) in values.iter().enumerate() {
            let style = if col_idx == 5 && row.commit_url.is_some() {
                3
            } else {
                2
            };
            xml.push_str(&inline_str_cell(
                &cell_ref(col_idx, excel_row),
                value,
                style,
            ));
        }
        xml.push_str("</row>");
    }
    xml.push_str("</sheetData>");

    let hyperlink_count = rows
        .iter()
        .filter(|row| row.commit_url.as_deref().is_some_and(|url| !url.is_empty()))
        .count();
    if hyperlink_count > 0 {
        xml.push_str("<hyperlinks>");
        let mut rid = 1;
        for (idx, row) in rows.iter().enumerate() {
            if row.commit_url.as_deref().is_some_and(|url| !url.is_empty()) {
                xml.push_str(&format!(
                    r#"<hyperlink ref="F{}" r:id="rId{}" tooltip="commit_url"/>"#,
                    idx + 2,
                    rid
                ));
                rid += 1;
            }
        }
        xml.push_str("</hyperlinks>");
    }

    xml.push_str(
        r#"<pageMargins left="0.75" right="0.75" top="1" bottom="1" header="0.5" footer="0.5"/></worksheet>"#,
    );
    xml
}

fn sheet_rels_xml(rows: &[CveFixComparisonRow]) -> String {
    let mut xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#
        .to_string();
    let mut rid = 1;
    for row in rows {
        if let Some(url) = row.commit_url.as_deref().filter(|url| !url.is_empty()) {
            xml.push_str(&format!(
                r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="{}" TargetMode="External"/>"#,
                rid,
                xml_escape(url)
            ));
            rid += 1;
        }
    }
    xml.push_str("</Relationships>");
    xml
}

fn inline_str_cell(cell_ref: &str, value: &str, style: usize) -> String {
    format!(
        r#"<c r="{cell_ref}" s="{style}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
        xml_escape(value)
    )
}

fn cell_ref(col_idx: usize, row_idx: usize) -> String {
    let col = (b'A' + col_idx as u8) as char;
    format!("{col}{row_idx}")
}

fn write_stored_zip(path: &Path, files: &[(String, Vec<u8>)]) -> Result<()> {
    let mut output = Vec::new();
    let mut central_directory = Vec::new();
    let dos_time = 0u16;
    let dos_date = (46u16 << 9) | (1u16 << 5) | 1u16;

    for (name, data) in files {
        let name_bytes = name.as_bytes();
        let offset = output.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;

        write_u32(&mut output, 0x0403_4b50)?;
        write_u16(&mut output, 20)?;
        write_u16(&mut output, 0)?;
        write_u16(&mut output, 0)?;
        write_u16(&mut output, dos_time)?;
        write_u16(&mut output, dos_date)?;
        write_u32(&mut output, crc)?;
        write_u32(&mut output, size)?;
        write_u32(&mut output, size)?;
        write_u16(&mut output, name_bytes.len() as u16)?;
        write_u16(&mut output, 0)?;
        output.write_all(name_bytes)?;
        output.write_all(data)?;

        write_u32(&mut central_directory, 0x0201_4b50)?;
        write_u16(&mut central_directory, 20)?;
        write_u16(&mut central_directory, 20)?;
        write_u16(&mut central_directory, 0)?;
        write_u16(&mut central_directory, 0)?;
        write_u16(&mut central_directory, dos_time)?;
        write_u16(&mut central_directory, dos_date)?;
        write_u32(&mut central_directory, crc)?;
        write_u32(&mut central_directory, size)?;
        write_u32(&mut central_directory, size)?;
        write_u16(&mut central_directory, name_bytes.len() as u16)?;
        write_u16(&mut central_directory, 0)?;
        write_u16(&mut central_directory, 0)?;
        write_u16(&mut central_directory, 0)?;
        write_u16(&mut central_directory, 0)?;
        write_u32(&mut central_directory, 0)?;
        write_u32(&mut central_directory, offset)?;
        central_directory.write_all(name_bytes)?;
    }

    let central_offset = output.len() as u32;
    let central_size = central_directory.len() as u32;
    output.extend_from_slice(&central_directory);

    write_u32(&mut output, 0x0605_4b50)?;
    write_u16(&mut output, 0)?;
    write_u16(&mut output, 0)?;
    write_u16(&mut output, files.len() as u16)?;
    write_u16(&mut output, files.len() as u16)?;
    write_u32(&mut output, central_size)?;
    write_u32(&mut output, central_offset)?;
    write_u16(&mut output, 0)?;

    fs::write(path, output)?;
    Ok(())
}

fn write_u16(output: &mut Vec<u8>, value: u16) -> Result<()> {
    output.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_u32(output: &mut Vec<u8>, value: u32) -> Result<()> {
    output.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn text_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn cve_list_field(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(normalize_identifier)
                .collect()
        })
        .unwrap_or_default()
}

fn extract_cve_identifiers(text: &str) -> Vec<String> {
    let mut identifiers = Vec::new();
    if let Ok(re) = Regex::new(r"(?i)CVE-\d{4}-\d{4,}") {
        identifiers.extend(
            re.find_iter(text)
                .map(|matched| matched.as_str().to_ascii_uppercase()),
        );
    }
    identifiers
}

fn normalize_identifier(identifier: &str) -> String {
    if identifier.to_ascii_lowercase().starts_with("cve-") {
        identifier.to_ascii_uppercase()
    } else {
        identifier.to_string()
    }
}

fn description_entry_for_commit(
    description: &str,
    commit_url: &str,
    identifiers: &[String],
) -> String {
    let commit_url = commit_url.trim();
    let suffix = if commit_url.is_empty() {
        String::new()
    } else if identifiers.is_empty() {
        format!("commit_url: {commit_url}")
    } else {
        format!("commit_url: {commit_url}({})", identifiers.join(","))
    };

    match (description.trim().is_empty(), suffix.is_empty()) {
        (true, true) => String::new(),
        (true, false) => suffix,
        (false, true) => description.trim().to_string(),
        (false, false) if description.contains(commit_url) => {
            description.replace(commit_url, suffix.trim_start_matches("commit_url: "))
        }
        (false, false) => format!("{}\n{}", description.trim(), suffix),
    }
}

fn upstream_fixed_version(commit: &Value, default_upstream_version: &str) -> String {
    first_non_empty(&[
        version_release_display(
            &text_field(commit, "UpstreamVersion"),
            &text_field(commit, "UpstreamRelease"),
        ),
        text_field(commit, "UpstreamVersionRelease"),
        default_upstream_version.to_string(),
    ])
}

fn is_newer_version_release(candidate: &str, current: &str) -> bool {
    if candidate.trim().is_empty() {
        return false;
    }
    if current.trim().is_empty() {
        return true;
    }

    compare_version_release(candidate, current) == Ordering::Greater
}

fn compare_version_release(left: &str, right: &str) -> Ordering {
    let (left_version, left_release) = split_version_release_display(left);
    let (right_version, right_release) = split_version_release_display(right);

    rpm_like_cmp(&left_version, &right_version)
        .then_with(|| rpm_like_cmp(&left_release, &right_release))
        .then_with(|| left.trim().cmp(right.trim()))
}

fn split_version_release_display(value: &str) -> (String, String) {
    let trimmed = value.trim();
    if let Some((version, release)) = trimmed.rsplit_once('-') {
        if !version.trim().is_empty() && !release.trim().is_empty() {
            return (version.trim().to_string(), release.trim().to_string());
        }
    }

    (trimmed.to_string(), String::new())
}

fn rpm_like_cmp(left: &str, right: &str) -> Ordering {
    let mut left_pos = 0;
    let mut right_pos = 0;

    loop {
        let left_segment = next_version_segment(left, left_pos);
        let right_segment = next_version_segment(right, right_pos);

        match (left_segment, right_segment) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (
                Some((left_is_num, left_value, next_left)),
                Some((right_is_num, right_value, next_right)),
            ) => {
                left_pos = next_left;
                right_pos = next_right;

                let ord = match (left_is_num, right_is_num) {
                    (true, false) => Ordering::Greater,
                    (false, true) => Ordering::Less,
                    (true, true) => compare_numeric_segment(left_value, right_value),
                    (false, false) => left_value.cmp(right_value),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

fn next_version_segment(value: &str, mut pos: usize) -> Option<(bool, &str, usize)> {
    let bytes = value.as_bytes();
    while pos < bytes.len() && !bytes[pos].is_ascii_alphanumeric() {
        pos += 1;
    }
    if pos >= bytes.len() {
        return None;
    }

    let is_numeric = bytes[pos].is_ascii_digit();
    let start = pos;
    while pos < bytes.len()
        && if is_numeric {
            bytes[pos].is_ascii_digit()
        } else {
            bytes[pos].is_ascii_alphabetic()
        }
    {
        pos += 1;
    }

    Some((is_numeric, &value[start..pos], pos))
}

fn compare_numeric_segment(left: &str, right: &str) -> Ordering {
    let left_trimmed = left.trim_start_matches('0');
    let right_trimmed = right.trim_start_matches('0');
    let left_normalized = if left_trimmed.is_empty() {
        "0"
    } else {
        left_trimmed
    };
    let right_normalized = if right_trimmed.is_empty() {
        "0"
    } else {
        right_trimmed
    };

    left_normalized
        .len()
        .cmp(&right_normalized.len())
        .then_with(|| left_normalized.cmp(right_normalized))
}

fn version_release_display(version: &str, release: &str) -> String {
    match (version.trim().is_empty(), release.trim().is_empty()) {
        (true, true) => String::new(),
        (false, true) => version.trim().to_string(),
        (true, false) => release.trim().to_string(),
        (false, false) if version.trim().ends_with(&format!("-{}", release.trim())) => {
            version.trim().to_string()
        }
        (false, false) => format!("{}-{}", version.trim(), release.trim()),
    }
}

fn normalize_system_version(system_version: &str) -> String {
    let trimmed = system_version.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lower = trimmed.to_ascii_lowercase();
    let normalized = lower
        .strip_prefix("ctyunos-")
        .or_else(|| lower.strip_prefix("ctyunos"))
        .unwrap_or(trimmed)
        .trim_start_matches('-')
        .trim();

    if normalized.is_empty() {
        "CTyunOS".to_string()
    } else {
        format!("CTyunOS{}", normalized)
    }
}

fn first_non_empty(values: &[String]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_default()
}

fn sanitize_filename(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.trim_matches('_').is_empty() {
        "package".to_string()
    } else {
        sanitized
    }
}

fn xml_escape(input: &str) -> String {
    input
        .chars()
        .filter(|ch| {
            matches!(
                *ch,
                '\u{9}' | '\u{A}' | '\u{D}' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}'
            )
        })
        .flat_map(|ch| match ch {
            '&' => "&amp;".chars().collect::<Vec<_>>(),
            '<' => "&lt;".chars().collect(),
            '>' => "&gt;".chars().collect(),
            '"' => "&quot;".chars().collect(),
            '\'' => "&apos;".chars().collect(),
            _ => vec![ch],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;

    #[test]
    #[serial]
    fn rows_include_commit_url_in_description() {
        let commits = vec![serde_json::json!({
            "Description": "Fix CVE-2026-1234",
            "ChangeType": "CVE",
            "CVEList": ["CVE-2026-1234"],
            "Url": "https://example.com/commit/abc",
            "CommitSha": "abcdef1234567890",
            "UpstreamVersion": "1.2.3-4",
        })];

        let rows = cve_fix_comparison_rows("bash", "ctyunos-22.06", "1.0-1", "", &commits);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cve_or_issue, "CVE-2026-1234");
        assert!(rows[0]
            .description
            .contains("commit_url: https://example.com/commit/abc(CVE-2026-1234)"));
    }

    #[test]
    #[serial]
    fn rows_generate_issue_numbers_for_non_cve_commits() {
        let issue_start = 12000;
        let commits = vec![
            serde_json::json!({
                "Description": "Fix parser bug",
                "ChangeType": "Bugfix",
                "CVEList": [],
                "Url": "https://example.com/commit/bug-1",
            }),
            serde_json::json!({
                "Description": "Backport upstream change",
                "ChangeType": "Backport",
                "CVEList": [],
                "Url": "https://example.com/commit/backport-1",
            }),
        ];

        let rows = cve_fix_comparison_rows_with_issue_start(
            "bash",
            "ctyunos-22.06",
            "1.0-1",
            "",
            &commits,
            issue_start,
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cve_or_issue, "ISSUE-12000,ISSUE-12001");
        assert!(rows[0]
            .description
            .contains("commit_url: https://example.com/commit/bug-1(ISSUE-12000)"));
        assert!(rows[0]
            .description
            .contains("commit_url: https://example.com/commit/backport-1(ISSUE-12001)"));
    }

    #[test]
    #[serial]
    fn rows_keep_cve_ids_and_generate_issue_for_mixed_rows() {
        let issue_start = 12010;
        let commits = vec![
            serde_json::json!({
                "Description": "Fix CVE-2026-1234",
                "ChangeType": "CVE",
                "CVEList": ["CVE-2026-1234"],
            }),
            serde_json::json!({
                "Description": "Fix issue without CVE",
                "ChangeType": "Bugfix",
                "CVEList": [],
            }),
        ];

        let rows = cve_fix_comparison_rows_with_issue_start(
            "bash",
            "ctyunos-22.06",
            "1.0-1",
            "",
            &commits,
            issue_start,
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cve_or_issue, "CVE-2026-1234,ISSUE-12010");
    }

    #[test]
    #[serial]
    fn rows_merge_same_package_and_keep_latest_upstream_version() {
        let issue_start = 12020;
        let commits = vec![
            serde_json::json!({
                "Description": "Fix CVE-2026-4878",
                "CVEList": ["CVE-2026-4878"],
                "Url": "https://example.com/commit/cve",
                "UpstreamVersion": "2.32",
                "UpstreamRelease": "10",
            }),
            serde_json::json!({
                "Description": "Fix issue",
                "CVEList": [],
                "Url": "https://example.com/commit/issue",
                "UpstreamVersion": "2.32",
                "UpstreamRelease": "11",
            }),
        ];

        let rows = cve_fix_comparison_rows_with_issue_start(
            "libcap",
            "ctyunos-22.06",
            "2.32-8",
            "",
            &commits,
            issue_start,
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cve_or_issue, "CVE-2026-4878,ISSUE-12020");
        assert_eq!(rows[0].upstream_fixed_version, "2.32-11");
        assert_eq!(rows[0].system_version, "CTyunOS22.06");
        assert!(rows[0]
            .description
            .contains("https://example.com/commit/cve(CVE-2026-4878)"));
        assert!(rows[0]
            .description
            .contains("https://example.com/commit/issue(ISSUE-12020)"));
    }

    #[test]
    #[serial]
    fn rows_merge_openssl_intermediate_fix_versions_into_latest_row() {
        let commits = vec![
            serde_json::json!({
                "Description": "fix CVE-2026-34180 CVE-2026-42766",
                "CVEList": ["CVE-2026-34180", "CVE-2026-42766"],
                "Url": "https://example.com/openssl/52",
                "UpstreamVersion": "3.0.12",
                "UpstreamRelease": "52",
            }),
            serde_json::json!({
                "Description": "fix CVE-2026-45446",
                "CVEList": ["CVE-2026-45446"],
                "Url": "https://example.com/openssl/51",
                "UpstreamVersion": "3.0.12",
                "UpstreamRelease": "51",
            }),
            serde_json::json!({
                "Description": "fix ISSUE-3022402",
                "CVEList": [],
                "Url": "https://example.com/openssl/49",
                "UpstreamVersion": "3.0.12",
                "UpstreamRelease": "49",
            }),
        ];

        let rows = cve_fix_comparison_rows_with_issue_start(
            "openssl",
            "CTyunOS25.07",
            "3.0.12-47",
            "3.0.12-31",
            &commits,
            3022402,
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].package, "openssl");
        assert_eq!(rows[0].upstream_fixed_version, "3.0.12-52");
        assert_eq!(rows[0].ctyunos_current_version, "3.0.12-47");
        assert_eq!(
            rows[0].cve_or_issue,
            "CVE-2026-34180,CVE-2026-42766,CVE-2026-45446,ISSUE-3022402"
        );
        assert!(rows[0]
            .description
            .contains("https://example.com/openssl/52"));
        assert!(rows[0]
            .description
            .contains("https://example.com/openssl/51"));
        assert!(rows[0]
            .description
            .contains("https://example.com/openssl/49"));
    }

    #[test]
    fn compare_version_release_orders_release_numbers_numerically() {
        assert_eq!(
            compare_version_release("3.0.12-52", "3.0.12-9"),
            Ordering::Greater
        );
        assert_eq!(
            compare_version_release("3.0.13-1", "3.0.12-99"),
            Ordering::Greater
        );
        assert_eq!(
            compare_version_release("1.1.1f-43", "1.1.1f-40"),
            Ordering::Greater
        );
    }

    #[test]
    #[serial]
    fn rows_for_inputs_merge_multiple_trackings_into_one_collection() {
        let inputs = vec![
            CveFixComparisonInput {
                tracking_id: 1,
                package_name: "bash".to_string(),
                system_version: "ctyunos-22.06".to_string(),
                ctyunos_current_version: "5.2-1".to_string(),
                default_upstream_version: "5.2-3".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": "Fix CVE-2026-1111",
                    "CVEList": ["CVE-2026-1111"],
                    "Url": "https://example.com/bash/commit/1",
                    "UpstreamVersion": "5.2",
                    "UpstreamRelease": "3",
                })],
            },
            CveFixComparisonInput {
                tracking_id: 2,
                package_name: "coreutils".to_string(),
                system_version: "CTyunOS25.07".to_string(),
                ctyunos_current_version: "9.5-2".to_string(),
                default_upstream_version: "9.5-4".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": "Fix non-CVE bug",
                    "CVEList": [],
                    "Url": "https://example.com/coreutils/commit/1",
                    "UpstreamVersionRelease": "9.5-4",
                })],
            },
        ];

        let rows = cve_fix_comparison_rows_for_inputs_with_issue_start(&inputs, 12030);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].package, "bash");
        assert_eq!(rows[0].cve_or_issue, "CVE-2026-1111");
        assert_eq!(rows[0].upstream_fixed_version, "5.2-3");
        assert_eq!(rows[0].system_version, "CTyunOS22.06");
        assert_eq!(rows[1].package, "coreutils");
        assert_eq!(rows[1].cve_or_issue, "ISSUE-12030");
        assert_eq!(rows[1].system_version, "CTyunOS25.07");
    }

    #[test]
    #[serial]
    fn rows_for_inputs_skip_default_blacklisted_system_versions() {
        let inputs = vec![
            CveFixComparisonInput {
                tracking_id: 1,
                package_name: "bash".to_string(),
                system_version: "ctyunos-2.0.1".to_string(),
                ctyunos_current_version: "5.2-1".to_string(),
                default_upstream_version: "5.2-3".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": "Fix CVE-2026-1111",
                    "CVEList": ["CVE-2026-1111"],
                })],
            },
            CveFixComparisonInput {
                tracking_id: 2,
                package_name: "coreutils".to_string(),
                system_version: "CTyunOS25.05".to_string(),
                ctyunos_current_version: "9.5-2".to_string(),
                default_upstream_version: "9.5-4".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": "Fix non-CVE bug",
                    "CVEList": [],
                })],
            },
            CveFixComparisonInput {
                tracking_id: 3,
                package_name: "grep".to_string(),
                system_version: "ctyunos-25.07".to_string(),
                ctyunos_current_version: "3.11-1".to_string(),
                default_upstream_version: "3.11-2".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": "Fix CVE-2026-2222",
                    "CVEList": ["CVE-2026-2222"],
                })],
            },
        ];

        let rows = cve_fix_comparison_rows_for_inputs(&inputs);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].package, "grep");
        assert_eq!(rows[0].system_version, "CTyunOS25.07");
    }

    #[test]
    #[serial]
    fn rows_for_inputs_allow_overriding_system_version_blacklist() {
        let _blacklist_guard = EnvVarGuard::set("TRACK_XLSX_SYSTEM_VERSION_BLACKLIST", "");
        let inputs = vec![CveFixComparisonInput {
            tracking_id: 1,
            package_name: "bash".to_string(),
            system_version: "ctyunos-2.0.1".to_string(),
            ctyunos_current_version: "5.2-1".to_string(),
            default_upstream_version: "5.2-3".to_string(),
            commit_reports: vec![serde_json::json!({
                "Description": "Fix CVE-2026-1111",
                "CVEList": ["CVE-2026-1111"],
            })],
        }];

        let rows = cve_fix_comparison_rows_for_inputs(&inputs);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].system_version, "CTyunOS2.0.1");
    }

    #[test]
    #[serial]
    fn row_volumes_for_inputs_split_after_default_thirty_packages() {
        let _blacklist_guard = EnvVarGuard::set("TRACK_XLSX_SYSTEM_VERSION_BLACKLIST", "");
        let _limit_guard = EnvVarGuard::set("TRACK_XLSX_MAX_PACKAGES", "30");
        let inputs = (0..31)
            .map(|idx| CveFixComparisonInput {
                tracking_id: idx,
                package_name: format!("pkg{idx:02}"),
                system_version: "ctyunos-25.07".to_string(),
                ctyunos_current_version: "1.0-1".to_string(),
                default_upstream_version: "1.0-2".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": format!("Fix issue {idx}"),
                    "CVEList": [],
                })],
            })
            .collect::<Vec<_>>();

        let volumes = cve_fix_comparison_row_volumes_for_inputs_with_issue_start(&inputs, 12040);
        let first_packages = volumes[0]
            .iter()
            .map(|row| row.package.as_str())
            .collect::<HashSet<_>>();
        let second_packages = volumes[1]
            .iter()
            .map(|row| row.package.as_str())
            .collect::<HashSet<_>>();

        assert_eq!(volumes.len(), 2);
        assert_eq!(first_packages.len(), 30);
        assert!(first_packages.contains("pkg00"));
        assert!(first_packages.contains("pkg29"));
        assert_eq!(second_packages.len(), 1);
        assert!(second_packages.contains("pkg30"));
    }

    #[test]
    #[serial]
    fn row_volumes_for_inputs_keep_existing_package_in_same_volume_after_limit_is_reached() {
        let _blacklist_guard = EnvVarGuard::set("TRACK_XLSX_SYSTEM_VERSION_BLACKLIST", "");
        let _limit_guard = EnvVarGuard::set("TRACK_XLSX_MAX_PACKAGES", "1");
        let inputs = vec![
            CveFixComparisonInput {
                tracking_id: 1,
                package_name: "bash".to_string(),
                system_version: "ctyunos-22.06".to_string(),
                ctyunos_current_version: "5.2-1".to_string(),
                default_upstream_version: "5.2-2".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": "Fix bash issue 1",
                    "CVEList": [],
                })],
            },
            CveFixComparisonInput {
                tracking_id: 2,
                package_name: "coreutils".to_string(),
                system_version: "ctyunos-22.06".to_string(),
                ctyunos_current_version: "9.5-1".to_string(),
                default_upstream_version: "9.5-2".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": "Fix coreutils issue",
                    "CVEList": [],
                })],
            },
            CveFixComparisonInput {
                tracking_id: 3,
                package_name: "bash".to_string(),
                system_version: "ctyunos-25.07".to_string(),
                ctyunos_current_version: "5.2-3".to_string(),
                default_upstream_version: "5.2-4".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": "Fix bash issue 2",
                    "CVEList": [],
                })],
            },
        ];

        let volumes = cve_fix_comparison_row_volumes_for_inputs_with_issue_start(&inputs, 12080);

        assert_eq!(volumes.len(), 2);
        assert_eq!(volumes[0].len(), 2);
        assert!(volumes[0].iter().all(|row| row.package == "bash"));
        assert!(volumes[0]
            .iter()
            .any(|row| row.system_version == "CTyunOS22.06"));
        assert!(volumes[0]
            .iter()
            .any(|row| row.system_version == "CTyunOS25.07"));
        assert_eq!(volumes[1].len(), 1);
        assert_eq!(volumes[1][0].package, "coreutils");
    }

    #[test]
    fn time_based_issue_start_number_is_bounded_and_time_progressive() {
        assert_eq!(
            time_based_issue_start_number(ISSUE_NUMBER_EPOCH_UNIX_SECS),
            ISSUE_START_NUMBER
        );
        assert_eq!(
            time_based_issue_start_number(ISSUE_NUMBER_EPOCH_UNIX_SECS + 60),
            ISSUE_START_NUMBER + 1
        );
        assert_eq!(
            time_based_issue_start_number(ISSUE_NUMBER_EPOCH_UNIX_SECS - 3600),
            ISSUE_START_NUMBER
        );
        assert_eq!(time_based_issue_start_number(i64::MAX), ISSUE_MAX_NUMBER);
    }

    #[test]
    #[serial]
    fn issue_number_reservation_uses_state_file_without_repeating() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("issue-number-state");
        let _state_guard = EnvVarGuard::set(
            "TRACK_XLSX_ISSUE_NUMBER_STATE_FILE",
            state_path.to_str().unwrap(),
        );

        let first = reserve_issue_number_block().unwrap();
        let second = reserve_issue_number_block().unwrap();

        assert!(first >= ISSUE_START_NUMBER);
        assert_eq!(second, first + ISSUE_RESERVATION_BLOCK_SIZE);
        assert_eq!(
            fs::read_to_string(state_path).unwrap().trim(),
            second.to_string()
        );
    }

    #[test]
    #[serial]
    fn round_writer_rewrites_same_file_as_inputs_are_appended() {
        let dir = tempdir().unwrap();
        let _artifact_dir_guard =
            EnvVarGuard::set("TRACK_REPORT_ARTIFACT_DIR", dir.path().to_str().unwrap());
        let _limit_guard = EnvVarGuard::set("TRACK_XLSX_MAX_PACKAGES", "30");
        let mut writer = RoundCveFixComparisonWriter::new();

        let first = writer
            .append_input(CveFixComparisonInput {
                tracking_id: 1,
                package_name: "bash".to_string(),
                system_version: "ctyunos-22.06".to_string(),
                ctyunos_current_version: "5.2-1".to_string(),
                default_upstream_version: "5.2-3".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": "Fix CVE-2026-1111",
                    "CVEList": ["CVE-2026-1111"],
                })],
            })
            .unwrap();
        let second = writer
            .append_input(CveFixComparisonInput {
                tracking_id: 2,
                package_name: "coreutils".to_string(),
                system_version: "ctyunos-25.07".to_string(),
                ctyunos_current_version: "9.5-2".to_string(),
                default_upstream_version: "9.5-4".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": "Fix non-CVE bug",
                    "CVEList": [],
                })],
            })
            .unwrap();

        assert_eq!(first.path, second.path);
        assert_eq!(first.rows, 1);
        assert_eq!(second.rows, 2);
        let bytes = fs::read(&second.path).unwrap();
        assert_eq!(&bytes[..4], b"PK\x03\x04");
    }

    #[test]
    #[serial]
    fn round_writer_creates_new_xlsx_volume_when_package_limit_is_exceeded() {
        let dir = tempdir().unwrap();
        let _artifact_dir_guard =
            EnvVarGuard::set("TRACK_REPORT_ARTIFACT_DIR", dir.path().to_str().unwrap());
        let _limit_guard = EnvVarGuard::set("TRACK_XLSX_MAX_PACKAGES", "1");
        let _state_guard = EnvVarGuard::set(
            "TRACK_XLSX_ISSUE_NUMBER_STATE_FILE",
            dir.path().join("issue-state").to_str().unwrap(),
        );
        let mut writer = RoundCveFixComparisonWriter::new();

        writer
            .append_input(CveFixComparisonInput {
                tracking_id: 1,
                package_name: "bash".to_string(),
                system_version: "ctyunos-22.06".to_string(),
                ctyunos_current_version: "5.2-1".to_string(),
                default_upstream_version: "5.2-3".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": "Fix bash issue",
                    "CVEList": [],
                })],
            })
            .unwrap();
        let second = writer
            .append_input(CveFixComparisonInput {
                tracking_id: 2,
                package_name: "coreutils".to_string(),
                system_version: "ctyunos-25.07".to_string(),
                ctyunos_current_version: "9.5-2".to_string(),
                default_upstream_version: "9.5-4".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": "Fix coreutils issue",
                    "CVEList": [],
                })],
            })
            .unwrap();

        assert_eq!(writer.artifacts().len(), 2);
        assert!(writer.artifacts()[0].path.contains("_part01.xlsx"));
        assert!(writer.artifacts()[1].path.contains("_part02.xlsx"));
        assert_eq!(writer.artifacts()[0].rows, 1);
        assert_eq!(writer.artifacts()[1].rows, 1);
        assert_eq!(second.path, writer.artifacts()[1].path);
        assert_eq!(
            &fs::read(&writer.artifacts()[0].path).unwrap()[..4],
            b"PK\x03\x04"
        );
        assert_eq!(
            &fs::read(&writer.artifacts()[1].path).unwrap()[..4],
            b"PK\x03\x04"
        );
    }

    #[test]
    #[serial]
    fn round_writer_keeps_issue_numbers_stable_when_rewriting_same_file() {
        let dir = tempdir().unwrap();
        let _artifact_dir_guard =
            EnvVarGuard::set("TRACK_REPORT_ARTIFACT_DIR", dir.path().to_str().unwrap());
        let _state_guard = EnvVarGuard::set(
            "TRACK_XLSX_ISSUE_NUMBER_STATE_FILE",
            dir.path().join("issue-state").to_str().unwrap(),
        );
        let mut writer = RoundCveFixComparisonWriter::new();

        writer
            .append_input(CveFixComparisonInput {
                tracking_id: 1,
                package_name: "bash".to_string(),
                system_version: "ctyunos-22.06".to_string(),
                ctyunos_current_version: "5.2-1".to_string(),
                default_upstream_version: "5.2-3".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": "Fix non-CVE bug",
                    "CVEList": [],
                    "Url": "https://example.com/bash/commit/1",
                })],
            })
            .unwrap();
        let first_issue = cve_fix_comparison_rows_for_inputs_with_issue_start(
            &writer.inputs,
            writer.issue_start_number,
        )[0]
        .cve_or_issue
        .clone();

        writer
            .append_input(CveFixComparisonInput {
                tracking_id: 2,
                package_name: "grep".to_string(),
                system_version: "ctyunos-25.07".to_string(),
                ctyunos_current_version: "3.11-1".to_string(),
                default_upstream_version: "3.11-2".to_string(),
                commit_reports: vec![serde_json::json!({
                    "Description": "Fix another non-CVE bug",
                    "CVEList": [],
                    "Url": "https://example.com/grep/commit/1",
                })],
            })
            .unwrap();
        let rewritten_rows = cve_fix_comparison_rows_for_inputs_with_issue_start(
            &writer.inputs,
            writer.issue_start_number,
        );

        assert_eq!(rewritten_rows[0].cve_or_issue, first_issue);
        assert_ne!(rewritten_rows[1].cve_or_issue, first_issue);
    }

    #[test]
    #[serial]
    fn writes_one_sheet_per_system_version() {
        let _blacklist_guard = EnvVarGuard::set("TRACK_XLSX_SYSTEM_VERSION_BLACKLIST", "");
        let dir = tempdir().unwrap();
        let path = dir.path().join("report.xlsx");
        let rows = vec![
            CveFixComparisonRow {
                package: "bash".to_string(),
                cve_or_issue: "CVE-2026-1234".to_string(),
                upstream_fixed_version: "5.2-3".to_string(),
                ctyunos_current_version: "5.2-1".to_string(),
                system_version: "CTyunOS22.06".to_string(),
                description: "Fix CVE".to_string(),
                xingkong_ticket_no: "97883".to_string(),
                commit_url: None,
            },
            CveFixComparisonRow {
                package: "grep".to_string(),
                cve_or_issue: "CVE-2026-2222".to_string(),
                upstream_fixed_version: "3.11-2".to_string(),
                ctyunos_current_version: "3.11-1".to_string(),
                system_version: "CTyunOS25.07".to_string(),
                description: "Fix CVE".to_string(),
                xingkong_ticket_no: "97883".to_string(),
                commit_url: None,
            },
        ];

        write_cve_fix_comparison_xlsx(&path, &rows).unwrap();
        let xlsx = String::from_utf8_lossy(&fs::read(path).unwrap()).to_string();

        assert!(xlsx.contains(r#"<sheet name="CTyunOS22.06" sheetId="1" r:id="rId1"/>"#));
        assert!(xlsx.contains(r#"<sheet name="CTyunOS25.07" sheetId="2" r:id="rId2"/>"#));
        assert!(xlsx.contains("xl/worksheets/sheet1.xml"));
        assert!(xlsx.contains("xl/worksheets/sheet2.xml"));
    }

    #[test]
    #[serial]
    fn writes_valid_zip_container_for_xlsx() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("report.xlsx");
        let rows = vec![CveFixComparisonRow {
            package: "bash".to_string(),
            cve_or_issue: "CVE-2026-1234".to_string(),
            upstream_fixed_version: "1.2.3-4".to_string(),
            ctyunos_current_version: "1.0-1".to_string(),
            system_version: "ctyunos-22.06".to_string(),
            description: "Fix CVE\ncommit_url: https://example.com/commit/abc".to_string(),
            xingkong_ticket_no: "97883".to_string(),
            commit_url: Some("https://example.com/commit/abc".to_string()),
        }];

        write_cve_fix_comparison_xlsx(&path, &rows).unwrap();
        let bytes = fs::read(path).unwrap();
        assert_eq!(&bytes[..4], b"PK\x03\x04");
    }

    struct EnvVarGuard {
        key: &'static str,
        old_value: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let old_value = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, old_value }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.old_value {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}
