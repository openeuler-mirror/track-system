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
