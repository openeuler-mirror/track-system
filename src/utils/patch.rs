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

//! Patch 文件解析工具
//!
//! 提供 patch 文件解析、CVE 编号提取、内容哈希计算等功能

use anyhow::{Context, Result};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Patch 文件解析器
pub struct PatchParser;

impl PatchParser {
    /// 从文件名提取 CVE 编号
    ///
    /// 支持的格式：
    /// - CVE-2023-1234.patch
    /// - fix-CVE-2023-1234.patch
    /// - 0001-CVE-2023-1234-fix-buffer-overflow.patch
    pub fn extract_cve_from_filename(filename: &str) -> Vec<String> {
        let re = Regex::new(r"CVE-\d{4}-\d{4,}").unwrap();
        re.find_iter(filename)
            .map(|m| m.as_str().to_string())
            .collect()
    }

    /// 从 patch 内容提取 CVE 编号
    ///
    /// 搜索 patch 文件内容中的 CVE 引用
    pub fn extract_cve_from_content(content: &str) -> Vec<String> {
        let re = Regex::new(r"CVE-\d{4}-\d{4,}").unwrap();
        let mut cves: Vec<String> = re
            .find_iter(content)
            .map(|m| m.as_str().to_string())
            .collect();

        // 去重
        cves.sort();
        cves.dedup();
        cves
    }

    /// 提取 patch 描述
    /// 提取 patch 描述
    ///
    /// 从 patch 文件的头部注释中提取描述信息
    pub fn extract_description(content: &str) -> Option<String> {
        let lines: Vec<&str> = content.lines().collect();

        // 查找第一个 diff 行之前的内容
        let mut description_lines = Vec::new();
        for line in lines {
            if line.starts_with("diff ") || line.starts_with("--- ") {
                break;
            }

            // 跳过空行和特殊标记
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("From ") || trimmed.starts_with("Date:") {
                continue;
            }

            // 处理 Subject 行，提取实际内容
            if trimmed.starts_with("Subject:") {
                if let Some(subject_content) = trimmed.strip_prefix("Subject:").map(|s| s.trim()) {
                    if !subject_content.is_empty() {
                        description_lines.push(subject_content);
                    }
                }
                continue;
            }

            description_lines.push(trimmed);
        }

        if description_lines.is_empty() {
            None
        } else {
            Some(description_lines.join(" "))
        }
    }

    /// 计算 patch 内容的 SHA256 哈希
    pub fn calculate_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// 从文件名提取补丁序号
    ///
    /// 支持的格式：
    /// - 0001-fix-bug.patch -> Some(1)
    /// - fix-bug.patch -> None
    pub fn extract_patch_number(filename: &str) -> Option<u32> {
        let re = Regex::new(r"^(\d{4})-").unwrap();
        re.captures(filename)
            .and_then(|cap| cap.get(1))
            .and_then(|m| m.as_str().parse::<u32>().ok())
    }

    /// 判断是否为 backport patch
    ///
    /// 通过文件名或内容判断是否为从上游回合的补丁
    pub fn is_backport_patch(filename: &str, content: &str) -> bool {
        let filename_lower = filename.to_lowercase();
        let content_lower = content.to_lowercase();

        // 检查文件名
        if filename_lower.contains("backport")
            || filename_lower.contains("upstream")
            || filename_lower.contains("cherry-pick")
        {
            return true;
        }

        // 检查内容
        if content_lower.contains("backport")
            || content_lower.contains("cherry-pick")
            || content_lower.contains("upstream commit")
        {
            return true;
        }

        false
    }

    /// 提取上游 commit SHA（如果是 backport patch）
    pub fn extract_upstream_commit(content: &str) -> Option<String> {
        // 查找常见的 commit 引用格式（不区分大小写）
        let content_lower = content.to_lowercase();

        let patterns = [
            r"upstream commit[:\s]+([0-9a-f]{7,40})",
            r"cherry-pick[:\s]+([0-9a-f]{7,40})",
            r"backport[:\s]+([0-9a-f]{7,40})",
            r"\(commit[:\s]+([0-9a-f]{7,40})\)",
        ];

        for pattern in &patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(cap) = re.captures(&content_lower) {
                    if let Some(m) = cap.get(1) {
                        return Some(m.as_str().to_string());
                    }
                }
            }
        }

        None
    }

    /// 读取并解析 patch 文件
    pub fn parse_file(path: &Path) -> Result<ParsedPatch> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("无法读取 patch 文件: {:?}", path))?;

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(Self::parse(&filename, &content))
    }

    /// 解析 patch 内容
    pub fn parse(filename: &str, content: &str) -> ParsedPatch {
        let cves_from_filename = Self::extract_cve_from_filename(filename);
        let cves_from_content = Self::extract_cve_from_content(content);

        // 合并并去重 CVE 列表
        let mut all_cves = cves_from_filename;
        all_cves.extend(cves_from_content);
        all_cves.sort();
        all_cves.dedup();

        ParsedPatch {
            filename: filename.to_string(),
            description: Self::extract_description(content),
            cve_ids: all_cves,
            content_hash: Self::calculate_hash(content),
            patch_number: Self::extract_patch_number(filename),
            is_backport: Self::is_backport_patch(filename, content),
            upstream_commit: Self::extract_upstream_commit(content),
            content: content.to_string(),
        }
    }
}

/// 解析后的 Patch 信息
#[derive(Debug, Clone)]
pub struct ParsedPatch {
    /// 文件名
    pub filename: String,
    /// 描述
    pub description: Option<String>,
    /// CVE 编号列表
    pub cve_ids: Vec<String>,
    /// 内容哈希
    pub content_hash: String,
    /// 补丁序号
    pub patch_number: Option<u32>,
    /// 是否为 backport patch
    pub is_backport: bool,
    /// 上游 commit SHA（如果是 backport）
    pub upstream_commit: Option<String>,
    /// 原始内容
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_cve_from_filename() {
        let filename = "CVE-2023-1234.patch";
        let cves = PatchParser::extract_cve_from_filename(filename);
        assert_eq!(cves.len(), 1);
        assert_eq!(cves[0], "CVE-2023-1234");

        let filename = "fix-CVE-2023-1234.patch";
        let cves = PatchParser::extract_cve_from_filename(filename);
        assert_eq!(cves.len(), 1);
        assert_eq!(cves[0], "CVE-2023-1234");

        let filename = "0001-CVE-2023-1234-fix-buffer-overflow.patch";
        let cves = PatchParser::extract_cve_from_filename(filename);
        assert_eq!(cves.len(), 1);
        assert_eq!(cves[0], "CVE-2023-1234");

        let filename = "no_cve_here.patch";
        let cves = PatchParser::extract_cve_from_filename(filename);
        assert!(cves.is_empty());
    }

    #[test]
    fn test_extract_cve_from_content() {
        let content = r#"
Subject: [PATCH] Fix CVE-2023-1234 and CVE-2023-5678
This patch fixes multiple security issues.
References:
- https://nvd.nist.gov/vuln/detail/CVE-2023-1234
- https://nvd.nist.gov/vuln/detail/CVE-2023-5678
"#;
        let cves = PatchParser::extract_cve_from_content(content);
        assert_eq!(cves.len(), 2);
        assert!(cves.contains(&"CVE-2023-1234".to_string()));
        assert!(cves.contains(&"CVE-2023-5678".to_string()));
    }

    #[test]
    fn test_extract_description() {
        let content = r#"
Subject: [PATCH] Fix buffer overflow

This patch fixes a buffer overflow vulnerability in the parser.
It ensures that input length is checked before processing.

diff --git a/parser.c b/parser.c
index 123456..789abc 100644
--- a/parser.c
+++ b/parser.c
"#;
        let description = PatchParser::extract_description(content);
        assert!(description.is_some());
        let desc = description.unwrap();
        assert!(desc.contains("Fix buffer overflow"));
        assert!(desc.contains("This patch fixes a buffer overflow"));
    }

    #[test]
    fn test_calculate_hash() {
        let content = "test content";
        let hash = PatchParser::calculate_hash(content);
        // echo -n "test content" | sha256sum
        assert_eq!(
