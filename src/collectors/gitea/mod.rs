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

use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;
use tracing::info;

use crate::collectors::{
    error::{ApiError, ApiResult},
    traits::{Branch, Collector, Commit, CommitsParams, FileContent, GitClient, Repository},
};

use self::models::{GiteaBranch, GiteaCommit, GiteaFileContent, GiteaRepository};

mod models;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u32 = 3;

/// Gitea API 客户端
pub struct GiteaClient {
    client: Client,
    token: String,
    base_url: String,
}

