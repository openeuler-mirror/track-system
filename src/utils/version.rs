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
    /// - 1.2.3+build.123
    pub fn parse(version_str: &str) -> Result<Version> {
        let raw = version_str.to_string();
        let version_str = version_str.trim();

        // 移除前缀 'v' 或 'V'
        let version_str = version_str
            .strip_prefix('v')
            .or_else(|| version_str.strip_prefix('V'))
            .unwrap_or(version_str);

        // 分离构建元数据（+号后面的部分）
        let (version_part, build) = if let Some(pos) = version_str.find('+') {
            let (v, b) = version_str.split_at(pos);
            (v, Some(b[1..].to_string()))
        } else {
            (version_str, None)
        };

        // 分离预发布标识（-号后面的部分）
        let (core_version, pre_release) = if let Some(pos) = version_part.find('-') {
            let (v, p) = version_part.split_at(pos);
            (v, Some(p[1..].to_string()))
        } else {
            (version_part, None)
        };

        // 解析核心版本号（major.minor.patch）
        let parts: Vec<&str> = core_version.split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return Err(anyhow!("无效的版本格式: {}", version_str));
        }

        let major = parts
            .first()
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(|| anyhow!("无法解析主版本号: {}", version_str))?;

        let minor = parts
            .get(1)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        let patch = parts
            .get(2)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        Ok(Version {
            major,
            minor,
            patch,
            pre_release,
            build,
            raw,
        })
    }

    /// 批量解析版本列表
    pub fn parse_list(version_strs: &[String]) -> Vec<Version> {
        version_strs
            .iter()
            .filter_map(|s| Self::parse(s).ok())
            .collect()
    }

    /// 查找最新稳定版本
    pub fn find_latest_stable(versions: &[Version]) -> Option<&Version> {
        versions
            .iter()
            .filter(|v| v.is_stable())
            .max_by(|a, b| a.cmp(b))
    }

    /// 查找最新版本（包括预发布版本）
    pub fn find_latest(versions: &[Version]) -> Option<&Version> {
        versions.iter().max_by(|a, b| a.cmp(b))
    }

    /// 查找比指定版本新的所有版本
    pub fn find_newer_versions<'a>(
        current: &Version,
        all_versions: &'a [Version],
    ) -> Vec<&'a Version> {
        all_versions
            .iter()
            .filter(|v| v.is_newer_than(current))
            .collect()
    }

    /// 计算版本落后数量
    pub fn count_versions_behind(current: &Version, all_versions: &[Version]) -> u32 {
        all_versions
            .iter()
            .filter(|v| v.is_stable() && v.is_newer_than(current))
            .count() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_creation() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(v.is_stable());
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn test_version_with_pre_release() {
        let v = Version::with_pre_release(1, 2, 3, "beta.1");
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(v.is_pre_release());
        assert_eq!(v.pre_release, Some("beta.1".to_string()));
        assert_eq!(v.to_string(), "1.2.3-beta.1");
    }

    #[test]
    fn test_version_comparison() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(1, 1, 0);
        let v3 = Version::new(2, 0, 0);
        let _v4 = Version::with_pre_release(2, 0, 0, "rc.1");

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);

        // Note: This implementation of Version comparison might depend on raw string if Ord is derived
        // Let's check distance_from logic which is manually implemented
        assert!(v2.is_newer_than(&v1));
        assert!(v1.is_older_than(&v2));
    }

    #[test]
    fn test_version_distance() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(1, 1, 0);
        let v3 = Version::new(2, 0, 0);

        // 1.1.0 - 1.0.0 = 100 (minor diff * 100)
        assert_eq!(v2.distance_from(&v1), 100);
        // 2.0.0 - 1.0.0 = 10000 (major diff * 10000)
        assert_eq!(v3.distance_from(&v1), 10000);
    }
}
