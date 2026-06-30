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

/// 变更类型定义
use serde::{Deserialize, Serialize};

/// 变更类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    /// CVE 安全漏洞修复
    CVE,
    /// Bug 修复
    Bugfix,
    /// 回合移植
    Backport,
    /// 未知类型
    Unknown,
}

impl ChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChangeType::CVE => "CVE",
            ChangeType::Bugfix => "Bugfix",
