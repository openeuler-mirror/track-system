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

//! 版本解析和对比工具
//!
//! 支持语义化版本（Semantic Versioning）和常见的版本格式

use anyhow::{anyhow, Result};
use std::cmp::Ordering;
use std::fmt;

/// 版本结构体
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    /// 主版本号
    pub major: u32,
    /// 次版本号
    pub minor: u32,
    /// 修订版本号
    pub patch: u32,
    /// 预发布标识（如 alpha, beta, rc）
    pub pre_release: Option<String>,
    /// 构建元数据
    pub build: Option<String>,
    /// 原始版本字符串
    pub raw: String,
}

impl Version {
    /// 创建新版本
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            pre_release: None,
            build: None,
            raw: format!("{}.{}.{}", major, minor, patch),
        }
    }

    /// 带预发布标识创建版本
    pub fn with_pre_release(
        major: u32,
        minor: u32,
        patch: u32,
        pre_release: impl Into<String>,
    ) -> Self {
        let pre = pre_release.into();
        Self {
            major,
            minor,
            patch,
            pre_release: Some(pre.clone()),
            build: None,
            raw: format!("{}.{}.{}-{}", major, minor, patch, pre),
        }
    }

    /// 是否为稳定版本（无预发布标识）
    pub fn is_stable(&self) -> bool {
        self.pre_release.is_none()
    }

    /// 是否为预发布版本
    pub fn is_pre_release(&self) -> bool {
        self.pre_release.is_some()
    }

    /// 获取版本距离（相对于另一个版本）
    /// 返回值：正数表示当前版本更新，负数表示当前版本更旧
    pub fn distance_from(&self, other: &Version) -> i32 {
        // 简化的距离计算：主要基于主版本号和次版本号
        let major_diff = (self.major as i32) - (other.major as i32);
        let minor_diff = (self.minor as i32) - (other.minor as i32);
        let patch_diff = (self.patch as i32) - (other.patch as i32);

        // 主版本号差异权重最高 (10000)，次版本号权重 (100)，补丁版本权重 (1)
        major_diff * 10000 + minor_diff * 100 + patch_diff
    }

    /// 是否比另一个版本新
    pub fn is_newer_than(&self, other: &Version) -> bool {
        self > other
    }

    /// 是否比另一个版本旧
    pub fn is_older_than(&self, other: &Version) -> bool {
        self < other
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        // 1. 比较主版本号
        match self.major.cmp(&other.major) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // 2. 比较次版本号
        match self.minor.cmp(&other.minor) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // 3. 比较修订版本号
        match self.patch.cmp(&other.patch) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // 4. 比较预发布标识
        // 规则：有预发布标识的版本 < 无预发布标识的版本
        match (&self.pre_release, &other.pre_release) {
            (None, None) => Ordering::Equal,
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(a), Some(b)) => compare_pre_release(a, b),
        }
    }
}

/// 比较预发布标识
fn compare_pre_release(a: &str, b: &str) -> Ordering {
    // 预发布标识优先级：alpha < beta < rc < 其他
    let priority_a = get_pre_release_priority(a);
    let priority_b = get_pre_release_priority(b);

    match priority_a.cmp(&priority_b) {
        Ordering::Equal => {
            // 如果类型相同，尝试提取数字进行比较
            let num_a = extract_pre_release_number(a);
            let num_b = extract_pre_release_number(b);
            match (num_a, num_b) {
                (Some(na), Some(nb)) => na.cmp(&nb),
                _ => a.cmp(b), // 字符串比较
            }
        }
        ord => ord,
    }
}

/// 获取预发布标识的优先级
fn get_pre_release_priority(pre: &str) -> u8 {
    let lower = pre.to_lowercase();
    if lower.starts_with("alpha") {
        1
    } else if lower.starts_with("beta") {
        2
    } else if lower.starts_with("rc") {
        3
    } else {
        4
    }
}

/// 从预发布标识中提取数字
fn extract_pre_release_number(pre: &str) -> Option<u32> {
    // 提取字符串中的数字部分
    let digits: String = pre.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// 版本解析器
pub struct VersionParser;

impl VersionParser {
    /// 解析版本字符串
    ///
    /// 支持的格式：
    /// - 1.2.3
    /// - 1.2.3-alpha
    /// - 1.2.3-beta.1
    /// - 1.2.3-rc.2
    /// - v1.2.3
