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
            .collect::<Vec<_>>()
            .join("┬");
        output.push_str(&format!("┌{}┐\n", top_border));

        // 渲染表头
        let header_line = headers
            .iter()
            .enumerate()
            .map(|(i, h)| format!(" {:<width$} ", h, width = col_widths[i]))
            .collect::<Vec<_>>()
            .join("│");

        if self.use_color {
            output.push_str(&format!("│{}│\n", header_line.bold()));
        } else {
            output.push_str(&format!("│{}│\n", header_line));
        }

        // 渲染表头分隔线
        let header_separator = col_widths
            .iter()
            .map(|w| "─".repeat(w + 2))
            .collect::<Vec<_>>()
            .join("┼");
        output.push_str(&format!("├{}┤\n", header_separator));

        // 渲染数据行
        for (idx, row) in rows.iter().enumerate() {
            let row_line = row
                .iter()
                .enumerate()
                .map(|(i, cell)| {
                    if i < col_widths.len() {
                        format!(" {:<width$} ", cell, width = col_widths[i])
                    } else {
                        format!(" {} ", cell)
                    }
                })
                .collect::<Vec<_>>()
                .join("│");
            output.push_str(&format!("│{}│\n", row_line));

            // 如果不是最后一行，添加行分隔符
            if idx < rows.len() - 1 {
                let row_separator = col_widths
                    .iter()
                    .map(|w| "─".repeat(w + 2))
                    .collect::<Vec<_>>()
                    .join("┼");
                output.push_str(&format!("├{}┤\n", row_separator));
            }
        }

        // 渲染底部边框
        let bottom_border = col_widths
            .iter()
            .map(|w| "─".repeat(w + 2))
            .collect::<Vec<_>>()
            .join("┴");
        output.push_str(&format!("└{}┘\n", bottom_border));

        Ok(output)
    }
}

impl Default for TableFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for TableFormatter {
    fn format<T: Serialize>(&self, data: &T) -> anyhow::Result<String> {
        // 将数据序列化为 JSON，然后转换为表格
        let json_value = serde_json::to_value(data)?;

        match json_value {
            serde_json::Value::Array(arr) => {
                if arr.is_empty() {
                    return Ok("No data".to_string());
                }

                // 提取表头（从第一个对象的键）
                if let Some(serde_json::Value::Object(first)) = arr.first() {
                    let headers: Vec<&str> = first.keys().map(|k| k.as_str()).collect();

                    // 提取数据行
                    let rows: Vec<Vec<String>> = arr
                        .iter()
                        .filter_map(|v| {
                            if let serde_json::Value::Object(obj) = v {
                                Some(
                                    headers
                                        .iter()
                                        .map(|h| {
                                            obj.get(*h)
                                                .map(format_json_value)
                                                .unwrap_or_else(|| "".to_string())
                                        })
                                        .collect(),
                                )
                            } else {
                                None
                            }
                        })
                        .collect();

                    self.render_simple(&headers, &rows)
                } else {
                    Ok("Invalid data format".to_string())
                }
            }
            serde_json::Value::Object(obj) => {
                // 单个对象，渲染为键值对表格
                let headers = vec!["Key", "Value"];
                let rows: Vec<Vec<String>> = obj
                    .iter()
                    .map(|(k, v)| vec![k.clone(), format_json_value(v)])
                    .collect();

                self.render_simple(&headers, &rows)
            }
            _ => Ok(format!("{}", json_value)),
        }
    }
}

/// 格式化 JSON 值为字符串
fn format_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_simple() {
        let formatter = TableFormatter::new().with_color(false);
        let headers = vec!["Name", "Age", "City"];
        let rows = vec![vec!["Alice", "30", "New York"], vec!["Bob", "25", "London"]];

        let result = formatter.render_simple(&headers, &rows).unwrap();
        assert!(result.contains("Name"));
        assert!(result.contains("Alice"));
