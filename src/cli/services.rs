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

//! CLI 服务工厂
//!
//! 提供统一的方式来创建和初始化各种服务和客户端

use anyhow::Result;

use crate::collectors::github::GitHubClient;

/// 创建 GitHub 客户端
pub fn create_github_client() -> Result<GitHubClient> {
    let token = std::env::var("GITHUB_TOKEN").unwrap_or_else(|_| "".to_string());
    Ok(GitHubClient::new(token)?)
}
