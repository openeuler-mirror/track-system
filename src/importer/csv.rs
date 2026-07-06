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

//! CSV 软件包导入器
//!
//! 从 CSV 文件批量导入软件包配置

use chrono::Utc;
use sea_orm::*;
use serde::Deserialize;
use std::path::Path;

use crate::entities::{packages, prelude::*};

/// CSV 记录结构
#[derive(Debug, Deserialize)]
pub struct PackageRecord {
    pub name: String,
    pub level: i32,
    #[serde(default)]
    pub sync_interval_hours: Option<i32>,
    #[serde(default)]
    pub l0_repo_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// 导入结果
#[derive(Debug)]
pub struct ImportResult {
    pub success: bool,
    pub stats: ImportStats,
    pub errors: Vec<String>,
}

/// 导入统计
#[derive(Debug, Default)]
pub struct ImportStats {
    pub total: usize,
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// CSV 导入器
pub struct CsvImporter<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> CsvImporter<'a> {
    /// 创建新的导入器
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// 从 CSV 文件导入软件包
    pub async fn import_from_file(&self, path: impl AsRef<Path>) -> anyhow::Result<ImportResult> {
        let path = path.as_ref();

        // 读取 CSV 文件
        let mut reader = csv::ReaderBuilder::new()
            .comment(Some(b'#'))
            .from_path(path)?;

        let mut stats = ImportStats::default();
        let mut errors = Vec::new();

        // 逐行处理
        for (line_num, result) in reader.deserialize().enumerate() {
            let line_num = line_num + 2; // +2 因为有标题行和从0开始计数
            stats.total += 1;

            match result {
