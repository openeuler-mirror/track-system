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

/// 元数据导入器
///
/// 功能：
/// - 从 JSON 文件导入元数据
/// - 支持冲突处理（跳过/更新）
/// - 批量导入验证
use chrono::{DateTime, Utc};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{classifier_job::ClassificationJobQueue, exporter::ExportedMetadata};

/// 导入选项
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// 遇到冲突时是否跳过
    #[allow(dead_code)]
    pub skip_on_conflict: bool,
    /// 遇到冲突时是否更新
    pub update_on_conflict: bool,
}

/// 导入结果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImportResult {
    /// 是否成功
    pub success: bool,
    /// 导入的软件包数量
    pub imported_packages: usize,
    /// 跳过的软件包数量
    pub skipped_packages: usize,
    /// 更新的软件包数量
    pub updated_packages: usize,
    /// 导入的发行版数量
    pub imported_distros: usize,
    /// 导入的跟踪配置数量
    pub imported_trackings: usize,
    /// 导入的 commit 数量
    pub imported_commits: usize,
    /// 错误信息
    pub error: Option<String>,
}

/// 元数据导入器
pub struct MetadataImporter<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> MetadataImporter<'a> {
    /// 创建新的导入器
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// 导入元数据
    pub async fn import<P: AsRef<Path>>(
        &self,
        path: P,
        options: &ImportOptions,
    ) -> Result<ImportResult, DbErr> {
        // 读取文件
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| DbErr::Custom(format!("File read error: {}", e)))?;

        // 解析 JSON
        let metadata: ExportedMetadata = serde_json::from_str(&content)
            .map_err(|e| DbErr::Custom(format!("JSON parse error: {}", e)))?;

        // 导入数据
        let mut result = ImportResult::default();

        // 导入软件包
        for pkg_json in &metadata.packages {
            match self.import_package(pkg_json, options).await? {
                PackageImportResult::Imported => result.imported_packages += 1,
                PackageImportResult::Updated => result.updated_packages += 1,
                PackageImportResult::Skipped => result.skipped_packages += 1,
            }
        }

        // 导入发行版
        for distro_json in &metadata.distros {
            if self.import_distro(distro_json, options).await? {
                result.imported_distros += 1;
            }
        }

        // 导入跟踪配置
        for tracking_json in &metadata.trackings {
            if self.import_tracking(tracking_json, options).await? {
                result.imported_trackings += 1;
            }
        }

        // 导入 commit 记录（如果有）
        if let Some(commits) = &metadata.commits {
            for commit_json in commits {
                if self.import_commit(commit_json).await? {
                    result.imported_commits += 1;
                }
            }
        }

        result.success = true;
        Ok(result)
    }

    /// 导入单个软件包
    async fn import_package(
        &self,
        pkg_json: &serde_json::Value,
        options: &ImportOptions,
    ) -> Result<PackageImportResult, DbErr> {
        use crate::entities::{packages, prelude::Packages};

        let name = pkg_json["name"].as_str().unwrap_or("");
        let level = pkg_json["level"].as_i64().unwrap_or(0) as i32;
        let sync_interval_hours = pkg_json["sync_interval_hours"].as_i64().unwrap_or(12) as i32;

        // 检查是否已存在
        let existing = Packages::find()
            .filter(packages::Column::Name.eq(name))
            .one(self.db)
            .await?;

        if let Some(existing_pkg) = existing {
            // 已存在，处理冲突
            if options.skip_on_conflict {
                return Ok(PackageImportResult::Skipped);
            } else if options.update_on_conflict {
                // 更新
                let mut pkg: packages::ActiveModel = existing_pkg.into();
                pkg.level = Set(level);
                pkg.sync_interval_hours = Set(sync_interval_hours);
                pkg.updated_at = Set(Utc::now());
                pkg.update(self.db).await?;
                return Ok(PackageImportResult::Updated);
            } else {
                // 默认跳过
                return Ok(PackageImportResult::Skipped);
            }
        }

        // 插入新记录
        let now = Utc::now();
        let package = packages::ActiveModel {
            name: Set(name.to_string()),
            level: Set(level),
            sync_interval_hours: Set(sync_interval_hours),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        package.insert(self.db).await?;
        Ok(PackageImportResult::Imported)
    }

    /// 导入单个发行版
    async fn import_distro(
        &self,
        distro_json: &serde_json::Value,
        _options: &ImportOptions,
    ) -> Result<bool, DbErr> {
        use crate::entities::{distros, prelude::Distros};

        let name = distro_json["name"].as_str().unwrap_or("");
        let version = distro_json["version"].as_str().unwrap_or("");
        let platform = distro_json["platform"].as_str().unwrap_or("");
        let base_url = distro_json["base_url"].as_str().unwrap_or("");

        // 检查是否已存在
        let existing = Distros::find()
            .filter(distros::Column::Name.eq(name))
            .filter(distros::Column::Version.eq(version))
            .one(self.db)
            .await?;

        if existing.is_some() {
            // 已存在，跳过
            return Ok(false);
        }

