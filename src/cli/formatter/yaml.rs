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

//! YAML 格式输出
//!
//! 提供 YAML 格式的序列化输出

use serde::Serialize;

use super::Formatter;

/// YAML 格式化器
pub struct YamlFormatter;

impl YamlFormatter {
    /// 创建新的 YAML 格式化器
    pub fn new() -> Self {
        Self
    }
}

impl Default for YamlFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for YamlFormatter {
    fn format<T: Serialize>(&self, data: &T) -> anyhow::Result<String> {
        Ok(serde_yaml::to_string(data)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_format_yaml() {
        let formatter = YamlFormatter::new();
        let data = json!({
            "name": "test",
            "value": 123
        });

        let result = formatter.format(&data).unwrap();
        assert!(result.contains("name:"));
        assert!(result.contains("test"));
        assert!(result.contains("value:"));
    }
}
