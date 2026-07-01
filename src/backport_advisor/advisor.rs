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
