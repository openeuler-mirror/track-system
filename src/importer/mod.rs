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

//! 导入模块
//!
//! 支持：
//! - CSV 文件批量导入软件包配置
//! - JSON/SQL 元数据导入（用于内外网同步）
//! - track-collector JSON 导入

pub mod csv;
pub mod metadata;
pub mod metadata_importer;

pub use csv::{CsvImporter, ImportResult, ImportStats};
pub use metadata::{ImportOptions, ImportResult as MetadataImportResult, MetadataImporter};
pub use metadata_importer::{
    CollectedMetadata, ImportResult as CollectorImportResult, MetadataImporter as CollectorImporter,
};
