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
