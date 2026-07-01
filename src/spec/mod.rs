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

use regex::Regex;
use std::collections::HashMap;

/// 解析后的 spec 版本信息
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecInfo {
    pub version: String,
    pub release: String,
}

#[derive(Debug, Default)]
struct SpecFile {
    version: String,
    release: String,
    macros: HashMap<String, String>,
}

impl SpecFile {
    fn parse(content: &str) -> Self {
        let mut spec = Self::default();

        let version_re = Regex::new(r"^Version:\s*(.+)$").unwrap();
        let release_re = Regex::new(r"^Release:\s*(.+)$").unwrap();
        let macro_define_re = Regex::new(r"^%(?:define|global)\s+(\w+)\s+(.+)$").unwrap();
        let macro_usage_re = Regex::new(r"%\{(\??)([\w_]+)\}").unwrap();

        for raw_line in content.lines() {
            let line = raw_line.trim();

