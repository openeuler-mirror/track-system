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
