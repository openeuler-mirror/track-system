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
