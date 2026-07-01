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

use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DiffReport {
    pub tracking_id: i32,
    pub generated_at: DateTime<Utc>,
    pub file_diff: Vec<FileDiff>,
    pub spec_diff: Option<SpecDiff>,
    pub summary: SummaryDiff,
}

#[derive(Debug, Clone, Serialize)]
pub struct SummaryDiff {
    pub l1_commits: usize,
    pub l2_commits: usize,
    pub l1_issues: usize,
    pub l2_issues: usize,
}

#[derive(Debug, Clone, Serialize)]
