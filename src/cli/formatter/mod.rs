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

//! 输出格式化模块
//!
//! 提供多种输出格式支持

pub mod json;
pub mod table;
pub mod yaml;

pub use json::JsonFormatter;
pub use table::TableFormatter;
pub use yaml::YamlFormatter;

use chrono::{DateTime, Local, Utc};
use serde::Serialize;

/// 输出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// 表格格式
    Table,
    /// JSON 格式
    Json,
    /// YAML 格式
    Yaml,
}

#[allow(clippy::should_implement_trait)]
impl OutputFormat {
    /// 从字符串解析输出格式
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "table" => Some(Self::Table),
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            _ => None,
        }
    }
}

/// 格式化器 trait
pub trait Formatter {
    /// 格式化输出
    fn format<T: Serialize>(&self, data: &T) -> anyhow::Result<String>;
}

pub fn format_datetime_local(dt: &DateTime<Utc>) -> String {
    dt.with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}
