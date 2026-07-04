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

            if let Some(caps) = macro_define_re.captures(line) {
                let name = caps.get(1).unwrap().as_str();
                let value = caps.get(2).unwrap().as_str().trim();
                spec.macros
                    .insert(name.to_string(), spec.expand_macros(value, &macro_usage_re));
                continue;
            }

            if let Some(caps) = version_re.captures(line) {
                let raw = caps.get(1).unwrap().as_str().trim();
                let expanded = spec.expand_macros(raw, &macro_usage_re);
                spec.version = SpecFile::format_version(&expanded);
                continue;
            }

            if let Some(caps) = release_re.captures(line) {
                let raw = caps.get(1).unwrap().as_str().trim();
                let expanded = spec.expand_macros(raw, &macro_usage_re);
                spec.release = SpecFile::format_version(&expanded);
            }
        }

        spec
    }

    fn expand_macros(&self, value: &str, macro_usage_re: &Regex) -> String {
        let mut expanded = value.to_string();
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 10;

        while macro_usage_re.is_match(&expanded) && iterations < MAX_ITERATIONS {
            let replaced = macro_usage_re
                .replace_all(&expanded, |caps: &regex::Captures| {
                    let optional = caps.get(1).is_some_and(|m| !m.as_str().is_empty());
                    let name = caps.get(2).unwrap().as_str();
                    match self.macros.get(name) {
                        Some(value) => value.clone(),
                        None if optional => String::new(),
                        None => format!("%{{{}}}", name),
                    }
                })
