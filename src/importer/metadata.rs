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
