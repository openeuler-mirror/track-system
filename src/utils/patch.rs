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
