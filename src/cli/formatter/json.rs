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

//! JSON 格式输出
//!
//! 提供 JSON 格式的序列化和美化输出

use serde::Serialize;

use super::Formatter;

/// JSON 格式化器
pub struct JsonFormatter {
    /// 是否美化输出
    pretty: bool,
}

impl JsonFormatter {
    /// 创建新的 JSON 格式化器
    pub fn new() -> Self {
        Self { pretty: true }
    }

    /// 设置是否美化输出
    pub fn with_pretty(mut self, pretty: bool) -> Self {
        self.pretty = pretty;
        self
    }

    /// 格式化为紧凑 JSON
    pub fn format_compact<T: Serialize>(&self, data: &T) -> anyhow::Result<String> {
        Ok(serde_json::to_string(data)?)
    }

    /// 格式化为美化 JSON
    pub fn format_pretty<T: Serialize>(&self, data: &T) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(data)?)
    }
}

impl Default for JsonFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for JsonFormatter {
    fn format<T: Serialize>(&self, data: &T) -> anyhow::Result<String> {
        if self.pretty {
            self.format_pretty(data)
        } else {
            self.format_compact(data)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_format_pretty() {
        let formatter = JsonFormatter::new();
        let data = json!({
            "name": "test",
            "value": 123
        });

        let result = formatter.format(&data).unwrap();
        assert!(result.contains("name"));
        assert!(result.contains("test"));
