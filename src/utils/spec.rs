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

    /// 对比两个 spec 文件
    pub fn compare(spec1: &ParsedSpec, spec2: &ParsedSpec) -> SpecComparison {
        SpecComparison {
            version_changed: spec1.version != spec2.version,
            version_diff: if spec1.version != spec2.version {
                Some((
                    spec1.version.clone().unwrap_or_default(),
                    spec2.version.clone().unwrap_or_default(),
                ))
            } else {
                None
            },
            build_requires_added: Self::find_added(&spec1.build_requires, &spec2.build_requires),
            build_requires_removed: Self::find_removed(
                &spec1.build_requires,
                &spec2.build_requires,
            ),
            requires_added: Self::find_added(&spec1.requires, &spec2.requires),
            requires_removed: Self::find_removed(&spec1.requires, &spec2.requires),
            configure_options_added: Self::find_added(
                &spec1.configure_options,
                &spec2.configure_options,
            ),
            configure_options_removed: Self::find_removed(
                &spec1.configure_options,
                &spec2.configure_options,
            ),
            sources_changed: spec1.sources != spec2.sources,
            patches_changed: spec1.patches != spec2.patches,
        }
    }

    /// 查找新增的项
    fn find_added(old_list: &[String], new_list: &[String]) -> Vec<String> {
        new_list
            .iter()
            .filter(|item| !old_list.contains(item))
            .cloned()
            .collect()
    }

    /// 查找删除的项
    fn find_removed(old_list: &[String], new_list: &[String]) -> Vec<String> {
        old_list
            .iter()
            .filter(|item| !new_list.contains(item))
            .cloned()
            .collect()
    }
}

/// spec 文件对比结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecComparison {
    /// 版本是否变化
    pub version_changed: bool,
    /// 版本差异（旧版本，新版本）
    pub version_diff: Option<(String, String)>,
    /// 新增的 BuildRequires
    pub build_requires_added: Vec<String>,
    /// 删除的 BuildRequires
    pub build_requires_removed: Vec<String>,
    /// 新增的 Requires
    pub requires_added: Vec<String>,
    /// 删除的 Requires
    pub requires_removed: Vec<String>,
    /// 新增的 configure 选项
    pub configure_options_added: Vec<String>,
    /// 删除的 configure 选项
    pub configure_options_removed: Vec<String>,
    /// Source 是否变化
    pub sources_changed: bool,
    /// Patch 是否变化
    pub patches_changed: bool,
}

impl SpecComparison {
    /// 生成差异摘要
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        if self.version_changed {
            if let Some((old_ver, new_ver)) = &self.version_diff {
                parts.push(format!("版本从 {} 变更为 {}", old_ver, new_ver));
            }
        }

        if !self.build_requires_added.is_empty() {
            parts.push(format!(
                "新增 {} 个 BuildRequires",
                self.build_requires_added.len()
            ));
        }

        if !self.build_requires_removed.is_empty() {
            parts.push(format!(
                "删除 {} 个 BuildRequires",
                self.build_requires_removed.len()
            ));
        }

        if !self.requires_added.is_empty() {
            parts.push(format!("新增 {} 个 Requires", self.requires_added.len()));
        }

        if !self.requires_removed.is_empty() {
            parts.push(format!("删除 {} 个 Requires", self.requires_removed.len()));
        }

        if !self.configure_options_added.is_empty() {
            parts.push(format!(
                "新增 {} 个 configure 选项",
                self.configure_options_added.len()
            ));
        }

        if !self.configure_options_removed.is_empty() {
            parts.push(format!(
                "删除 {} 个 configure 选项",
                self.configure_options_removed.len()
            ));
        }

        if self.sources_changed {
            parts.push("Source 文件列表变化".to_string());
        }

        if self.patches_changed {
            parts.push("Patch 文件列表变化".to_string());
        }

        if parts.is_empty() {
            "无差异".to_string()
        } else {
            parts.join("；")
        }
    }

    /// 是否有重要变更
    pub fn has_significant_changes(&self) -> bool {
        self.version_changed
            || !self.build_requires_added.is_empty()
            || !self.build_requires_removed.is_empty()
            || !self.configure_options_added.is_empty()
            || !self.configure_options_removed.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_spec() {
        let content = r#"
Name: nginx
Version: 1.22.0
Release: 1%{?dist}
Summary: High performance web server
License: BSD
URL: https://nginx.org/

Source0: nginx-1.22.0.tar.gz
Patch0: 0001-fix-bug.patch

BuildRequires: gcc, make
Requires: openssl

%description
Nginx is a web server.

%build
%configure --with-http_ssl_module --enable-threads
make %{?_smp_mflags}

%install
make install DESTDIR=%{buildroot}

%files
%{_bindir}/nginx

%changelog
* Mon Jan 01 2024 Test <test@example.com> - 1.22.0-1
- Initial package
"#;

        let spec = SpecParser::parse(content).unwrap();

        assert_eq!(spec.name, Some("nginx".to_string()));
        assert_eq!(spec.version, Some("1.22.0".to_string()));
        assert_eq!(spec.release, Some("1%{?dist}".to_string()));
        assert_eq!(spec.license, Some("BSD".to_string()));
        assert_eq!(spec.sources.len(), 1);
        assert_eq!(spec.patches.len(), 1);
        assert_eq!(spec.build_requires.len(), 2);
        assert!(spec.build_requires.contains(&"gcc".to_string()));
        assert!(spec.build_requires.contains(&"make".to_string()));
        assert_eq!(spec.requires.len(), 1);
        assert_eq!(spec.configure_options.len(), 2);
        assert!(spec
            .configure_options
            .contains(&"--with-http_ssl_module".to_string()));
        assert!(spec
            .configure_options
            .contains(&"--enable-threads".to_string()));
    }

    #[test]
    fn test_extract_version() {
        let spec = ParsedSpec {
            name: Some("nginx".to_string()),
            version: Some("1.22.0".to_string()),
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

        let version = SpecParser::extract_version(&spec).unwrap();
        assert_eq!(version, "1.22.0");
    }

    #[test]
    fn test_compare_specs() {
