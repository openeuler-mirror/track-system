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

//! 流水线状态管理
//!
//! 负责跟踪和管理流水线执行状态

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{info, warn};

use crate::entities::{prelude::*, sync_jobs};

use super::pipeline_executor::{JobProgress, PipelineStage};

/// 流水线状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineState {
    pub job_id: i64,
    pub tracking_id: i32,
    pub current_stage: Option<PipelineStage>,
    pub completed_stages: Vec<PipelineStage>,
    pub stage_start_times: HashMap<PipelineStage, DateTime<Utc>>,
    pub can_cancel: bool,
    pub cancel_requested: bool,
}

impl PipelineState {
    pub fn new(job_id: i64, tracking_id: i32) -> Self {
