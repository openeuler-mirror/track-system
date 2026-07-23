/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

//! Ecosystem CLI 命令实现（基于 API）

use anyhow::{anyhow, bail, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::client::ApiClient;
use crate::cli::dto::{
    CreateEcosystemTargetRequest, EcosystemRefreshResultDto, EcosystemReportDto,
    EcosystemTargetDto, UpdateEcosystemTargetRequest,
};
use crate::cli::formatter::format_datetime_local;
