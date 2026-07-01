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
                Ok(record) => match self.import_package(record).await {
                    Ok(created) => {
                        if created {
                            stats.created += 1;
                        } else {
                            stats.updated += 1;
                        }
                    }
                    Err(e) => {
                        stats.failed += 1;
                        errors.push(format!("第 {} 行: {}", line_num, e));
                    }
                },
                Err(e) => {
                    stats.failed += 1;
                    errors.push(format!("第 {} 行解析错误: {}", line_num, e));
                }
            }
        }

        Ok(ImportResult {
            success: errors.is_empty(),
            stats,
            errors,
        })
    }

    /// 导入单个软件包
    async fn import_package(&self, record: PackageRecord) -> anyhow::Result<bool> {
        // 验证等级
        if !(1..=3).contains(&record.level) {
            return Err(anyhow::anyhow!("等级必须是 1、2 或 3"));
        }

        // 计算同步间隔（使用自定义值或默认值）
        let sync_interval_hours = record.sync_interval_hours.unwrap_or({
            match record.level {
                1 => 6,  // 关键软件 6 小时
                2 => 12, // 重要软件 12 小时
                3 => 24, // 普通软件 24 小时
                _ => 12, // 默认 12 小时
            }
        });

        let now = Utc::now();

        // 检查是否已存在
        let existing = Packages::find()
            .filter(packages::Column::Name.eq(&record.name))
            .one(self.db)
            .await?;

        let created = if let Some(pkg) = existing {
            // 更新现有软件包
            let mut active: packages::ActiveModel = pkg.into();
            active.level = Set(record.level);
            active.sync_interval_hours = Set(sync_interval_hours);
            active.l0_repo_url = Set(record.l0_repo_url);
            active.description = Set(record.description);
            active.updated_at = Set(now);
            active.update(self.db).await?;
            false
        } else {
            // 创建新软件包
            let package = packages::ActiveModel {
                name: Set(record.name),
                level: Set(record.level),
                sync_interval_hours: Set(sync_interval_hours),
                l0_repo_url: Set(record.l0_repo_url),
                description: Set(record.description),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            };
            package.insert(self.db).await?;
            true
        };

        Ok(created)
    }

    /// 导入 CSV 字符串（用于测试）
