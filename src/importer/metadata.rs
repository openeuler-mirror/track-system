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
