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

//! 表格格式输出
//!
//! 提供美观的表格格式输出

use colored::Colorize;
use serde::Serialize;
use std::fmt::Display;

use super::Formatter;

/// 表格格式化器
pub struct TableFormatter {
    /// 是否使用颜色
    use_color: bool,
}

impl TableFormatter {
    /// 创建新的表格格式化器
    pub fn new() -> Self {
        Self { use_color: true }
    }

    /// 设置是否使用颜色
    pub fn with_color(mut self, use_color: bool) -> Self {
        self.use_color = use_color;
        self
    }

    /// 渲染简单表格
    pub fn render_simple<T: Display>(
        &self,
        headers: &[&str],
        rows: &[Vec<T>],
    ) -> anyhow::Result<String> {
        if headers.is_empty() {
