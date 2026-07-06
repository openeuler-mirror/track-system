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
        Self {
            job_id,
            tracking_id,
            current_stage: None,
            completed_stages: Vec::new(),
            stage_start_times: HashMap::new(),
            can_cancel: true,
            cancel_requested: false,
        }
    }

    pub fn start_stage(&mut self, stage: PipelineStage) {
        self.current_stage = Some(stage);
        self.stage_start_times.insert(stage, Utc::now());
    }

    pub fn complete_stage(&mut self, stage: PipelineStage) {
        self.completed_stages.push(stage);
        self.current_stage = None;
    }

    pub fn request_cancel(&mut self) {
        self.cancel_requested = true;
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_requested
    }

    pub fn progress_percent(&self) -> f32 {
        let total_stages = PipelineStage::all_stages().len() as f32;
        let completed = self.completed_stages.len() as f32;
        (completed / total_stages) * 100.0
    }

    pub fn to_job_progress(&self, status: String) -> JobProgress {
        JobProgress {
            job_id: self.job_id,
            tracking_id: self.tracking_id,
            current_stage: self.current_stage,
            completed_stages: self.completed_stages.clone(),
            progress_percent: self.progress_percent(),
            status,
        }
    }
}

/// 流水线状态管理器
pub struct PipelineStateManager {
    db: Arc<DatabaseConnection>,
    states: Arc<RwLock<HashMap<i64, PipelineState>>>,
}

impl PipelineStateManager {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 创建新的流水线状态
    pub fn create_state(&self, job_id: i64, tracking_id: i32) -> Result<()> {
        let mut states = self.states.write().unwrap();
        states.insert(job_id, PipelineState::new(job_id, tracking_id));
        Ok(())
    }

    /// 更新当前阶段
    pub fn start_stage(&self, job_id: i64, stage: PipelineStage) -> Result<()> {
        let mut states = self.states.write().unwrap();
        if let Some(state) = states.get_mut(&job_id) {
            state.start_stage(stage);
        }
        Ok(())
    }

    /// 完成阶段
    pub fn complete_stage(&self, job_id: i64, stage: PipelineStage) -> Result<()> {
        let mut states = self.states.write().unwrap();
        if let Some(state) = states.get_mut(&job_id) {
            state.complete_stage(stage);
        }
        Ok(())
    }

    /// 请求取消
    pub fn request_cancel(&self, job_id: i64) -> Result<()> {
        let mut states = self.states.write().unwrap();
        if let Some(state) = states.get_mut(&job_id) {
            state.request_cancel();
            info!(job_id = job_id, "流水线取消请求已记录");
        } else {
            warn!(job_id = job_id, "流水线状态不存在");
        }
        Ok(())
    }

    /// 检查是否已取消
    pub fn is_cancelled(&self, job_id: i64) -> bool {
        let states = self.states.read().unwrap();
        states
            .get(&job_id)
            .map(|s| s.is_cancelled())
            .unwrap_or(false)
    }

    /// 获取任务进度
    pub async fn get_progress(&self, job_id: i64) -> Result<JobProgress> {
        // 从内存获取状态
        let state = {
            let states = self.states.read().unwrap();
            states.get(&job_id).cloned()
        };

        if let Some(state) = state {
            // 从数据库获取最新状态
            let job = SyncJobs::find_by_id(job_id)
                .one(self.db.as_ref())
                .await?
                .ok_or_else(|| anyhow::anyhow!("SyncJob {} 不存在", job_id))?;

            Ok(state.to_job_progress(job.status))
        } else {
            // 如果内存中没有,从数据库读取
