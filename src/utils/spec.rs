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

//! RPM spec 文件解析工具
//!
//! 提供 spec 文件的解析功能，提取版本、依赖、配置选项等关键信息

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 解析后的 spec 文件
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParsedSpec {
    /// 软件包名称
    pub name: Option<String>,
    /// 版本号
    pub version: Option<String>,
    /// Release 号
    pub release: Option<String>,
    /// Summary
    pub summary: Option<String>,
    /// License
    pub license: Option<String>,
    /// URL
    pub url: Option<String>,
    /// Source 列表
    pub sources: Vec<String>,
    /// Patch 列表
    pub patches: Vec<String>,
    /// BuildRequires 列表
    pub build_requires: Vec<String>,
    /// Requires 列表
    pub requires: Vec<String>,
    /// %configure 选项
    pub configure_options: Vec<String>,
    /// %build 部分内容
    pub build_section: Option<String>,
    /// %install 部分内容
    pub install_section: Option<String>,
    /// 所有宏定义
    pub macros: HashMap<String, String>,
}

/// spec 文件解析器
pub struct SpecParser;

impl SpecParser {
    /// 解析 spec 文件内容
    pub fn parse(content: &str) -> Result<ParsedSpec> {
        let mut spec = ParsedSpec {
            name: None,
            version: None,
            release: None,
            summary: None,
            license: None,
            url: None,
            sources: Vec::new(),
            patches: Vec::new(),
            build_requires: Vec::new(),
            requires: Vec::new(),
            configure_options: Vec::new(),
            build_section: None,
            install_section: None,
            macros: HashMap::new(),
        };

        let mut current_section: Option<String> = None;
        let mut section_content = String::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // 检测 section 开始（只检测 section 标记，不包括宏）
            if trimmed.starts_with('%')
                && !trimmed.starts_with("%{")
                && !trimmed.starts_with("%configure")
                && !trimmed.starts_with("%make")
                && !trimmed.starts_with("%cmake")
            {
                // 检查是否是 section 标记
                let is_section = trimmed.starts_with("%build")
                    || trimmed.starts_with("%install")
                    || trimmed.starts_with("%prep")
                    || trimmed.starts_with("%files")
                    || trimmed.starts_with("%changelog")
                    || trimmed.starts_with("%description")
                    || trimmed.starts_with("%package")
                    || trimmed.starts_with("%pre")
                    || trimmed.starts_with("%post")
                    || trimmed.starts_with("%preun")
                    || trimmed.starts_with("%postun");

                if is_section {
                    // 保存上一个 section
                    if let Some(section) = current_section.take() {
                        Self::save_section(&mut spec, &section, &section_content);
                        section_content.clear();
                    }

                    // 开始新 section
                    if trimmed.starts_with("%build") {
                        current_section = Some("build".to_string());
                    } else if trimmed.starts_with("%install") {
                        current_section = Some("install".to_string());
                    } else {
                        // 其他 section，暂时忽略
                        current_section = Some("other".to_string());
                    }
                    continue;
                }
            }

            // 如果在 section 中，收集内容
            if current_section.is_some() {
                section_content.push_str(line);
                section_content.push('\n');
                continue;
            }

            // 跳过空行和注释（只在头部区域）
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // 解析头部字段
            if let Some((key, value)) = Self::parse_header_line(trimmed) {
                match key.as_str() {
                    "Name" => spec.name = Some(value),
                    "Version" => spec.version = Some(value),
                    "Release" => spec.release = Some(value),
                    "Summary" => spec.summary = Some(value),
                    "License" => spec.license = Some(value),
                    "URL" => spec.url = Some(value),
                    "BuildRequires" => {
                        spec.build_requires
                            .extend(Self::parse_dependency_list(&value));
                    }
                    "Requires" => {
                        spec.requires.extend(Self::parse_dependency_list(&value));
                    }
                    _ => {
                        // 检查是否是 Source 或 Patch
                        if key.starts_with("Source") {
                            spec.sources.push(value);
                        } else if key.starts_with("Patch") {
                            spec.patches.push(value);
                        } else if key.starts_with("%define") || key.starts_with("%global") {
                            // 宏定义
                            if let Some((macro_name, macro_value)) = Self::parse_macro(&value) {
                                spec.macros.insert(macro_name, macro_value);
                            }
                        }
                    }
                }
            }
        }

        // 保存最后一个 section
        if let Some(section) = current_section {
            Self::save_section(&mut spec, &section, &section_content);
        }

        Ok(spec)
    }

    /// 解析头部行（Key: Value 格式）
    fn parse_header_line(line: &str) -> Option<(String, String)> {
        if let Some(pos) = line.find(':') {
            let key = line[..pos].trim().to_string();
            let value = line[pos + 1..].trim().to_string();
            Some((key, value))
        } else {
            None
        }
    }

    /// 解析依赖列表（可能包含逗号分隔）
    fn parse_dependency_list(value: &str) -> Vec<String> {
        value
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// 解析宏定义
    fn parse_macro(value: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = value.splitn(2, ' ').collect();
        if parts.len() == 2 {
            Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
        } else {
            None
        }
    }

    /// 保存 section 内容
    fn save_section(spec: &mut ParsedSpec, section: &str, content: &str) {
        match section {
            "build" => {
                spec.build_section = Some(content.to_string());
                // 提取 %configure 选项
                spec.configure_options = Self::extract_configure_options(content);
            }
            "install" => {
                spec.install_section = Some(content.to_string());
            }
            _ => {}
        }
    }

    /// 从 %build section 提取 %configure 选项
    fn extract_configure_options(content: &str) -> Vec<String> {
        let mut options = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("%configure") || trimmed.starts_with("./configure") {
                // 提取 configure 后面的所有选项
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                for part in parts.iter().skip(1) {
                    // 跳过 %configure 或 ./configure
                    // 只保留以 -- 开头的选项（标准的 configure 选项格式）
                    if part.starts_with("--") {
                        options.push(part.to_string());
                    }
                }
            }
        }

        options
    }

    /// 提取版本号（处理宏展开）
    pub fn extract_version(spec: &ParsedSpec) -> Result<String> {
        spec.version
            .clone()
            .ok_or_else(|| anyhow!("spec 文件中未找到 Version 字段"))
    }
