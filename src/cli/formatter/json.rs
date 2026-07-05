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
