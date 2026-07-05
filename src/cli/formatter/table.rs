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
            return Ok(String::new());
        }

        let mut output = String::new();

        // 计算每列的最大宽度
        let mut col_widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();

        for row in rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_widths.len() {
                    let cell_len = cell.to_string().len();
                    if cell_len > col_widths[i] {
                        col_widths[i] = cell_len;
                    }
                }
            }
        }

        // 渲染表头
        let header_line = headers
            .iter()
            .enumerate()
            .map(|(i, h)| format!("{:<width$}", h, width = col_widths[i]))
            .collect::<Vec<_>>()
            .join("  ");

        if self.use_color {
            output.push_str(&header_line.bold().to_string());
        } else {
            output.push_str(&header_line);
        }
        output.push('\n');

        // 渲染分隔线
        let separator = col_widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("  ");
        output.push_str(&separator);
        output.push('\n');

        // 渲染数据行
        for row in rows {
            let row_line = row
                .iter()
                .enumerate()
                .map(|(i, cell)| {
                    if i < col_widths.len() {
                        format!("{:<width$}", cell, width = col_widths[i])
                    } else {
                        cell.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("  ");
            output.push_str(&row_line);
            output.push('\n');
        }

        Ok(output)
    }

    /// 渲染带边框的表格
    pub fn render_bordered<T: Display>(
        &self,
        headers: &[&str],
        rows: &[Vec<T>],
    ) -> anyhow::Result<String> {
        if headers.is_empty() {
            return Ok(String::new());
        }

        let mut output = String::new();

        // 计算每列的最大宽度
        let mut col_widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();

        for row in rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_widths.len() {
                    let cell_len = cell.to_string().len();
                    if cell_len > col_widths[i] {
                        col_widths[i] = cell_len;
                    }
                }
            }
        }

        // 渲染顶部边框
        let top_border = col_widths
            .iter()
            .map(|w| "─".repeat(w + 2))
