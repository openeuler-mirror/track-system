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
            let job = SyncJobs::find_by_id(job_id)
                .one(self.db.as_ref())
                .await?
                .ok_or_else(|| anyhow::anyhow!("SyncJob {} 不存在", job_id))?;

            Ok(JobProgress {
                job_id,
                tracking_id: job.tracking_id,
                current_stage: None,
                completed_stages: vec![],
                progress_percent: 0.0,
                status: job.status,
            })
        }
    }

    /// 更新数据库中的任务状态
    pub async fn update_job_status(&self, job_id: i64, status: &str) -> Result<()> {
        let job = SyncJobs::find_by_id(job_id)
            .one(self.db.as_ref())
            .await?
            .ok_or_else(|| anyhow::anyhow!("SyncJob {} 不存在", job_id))?;

        let mut active_job: sync_jobs::ActiveModel = job.into();
        active_job.status = Set(status.to_string());
        active_job.updated_at = Set(Utc::now());

        active_job
            .update(self.db.as_ref())
            .await
            .context("更新任务状态失败")?;

        Ok(())
    }

    /// 清理已完成的状态
    pub fn cleanup_state(&self, job_id: i64) {
        let mut states = self.states.write().unwrap();
        states.remove(&job_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::pipeline_executor::PipelineStage;

    #[test]
    fn test_pipeline_state_lifecycle() {
        let mut state = PipelineState::new(1, 100);
        assert_eq!(state.job_id, 1);
        assert_eq!(state.tracking_id, 100);
        assert!(state.current_stage.is_none());
        assert!(state.completed_stages.is_empty());

        // Start a stage
        state.start_stage(PipelineStage::L1Ingestion);
        assert_eq!(state.current_stage, Some(PipelineStage::L1Ingestion));
        assert!(state
            .stage_start_times
            .contains_key(&PipelineStage::L1Ingestion));

        // Complete the stage
        state.complete_stage(PipelineStage::L1Ingestion);
        assert!(state.current_stage.is_none());
        assert_eq!(state.completed_stages.len(), 1);
        assert_eq!(state.completed_stages[0], PipelineStage::L1Ingestion);

        // Progress calculation (approximate, depends on total stages)
        assert!(state.progress_percent() > 0.0);
    }

    #[test]
    fn test_pipeline_cancellation() {
        let mut state = PipelineState::new(1, 100);
        assert!(!state.is_cancelled());

        state.request_cancel();
        assert!(state.is_cancelled());
    }

    #[tokio::test]
    async fn test_get_progress_from_db_when_not_in_memory() {
        use crate::entities::sync_jobs;
        use sea_orm::{DatabaseBackend, MockDatabase};

        let job_model = sync_jobs::Model {
            id: 1,
            tracking_id: 100,
            job_kind: "sync".to_string(),
            scheduled_at: Utc::now(),
            started_at: Some(Utc::now()),
            finished_at: None,
            status: "completed".to_string(),
            error: None,
            attempt_count: 0,
            priority: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results(vec![vec![job_model]])
            .into_connection();
        let db = Arc::new(db);
        let manager = PipelineStateManager::new(db);

        // Not in memory
        let progress = manager.get_progress(1).await.unwrap();
        assert_eq!(progress.job_id, 1);
        assert_eq!(progress.status, "completed");
        assert_eq!(progress.progress_percent, 0.0);
    }

    #[tokio::test]
    async fn test_update_job_status() {
        use crate::entities::sync_jobs;
        use sea_orm::{DatabaseBackend, MockDatabase};

        let job_model = sync_jobs::Model {
            id: 1,
            tracking_id: 100,
            job_kind: "sync".to_string(),
            scheduled_at: Utc::now(),
            started_at: Some(Utc::now()),
            finished_at: None,
            status: "running".to_string(),
            error: None,
            attempt_count: 0,
            priority: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results(vec![vec![job_model.clone()]])
            .append_query_results(vec![vec![job_model]]) // update returning
            .into_connection();
        let db = Arc::new(db);
        let manager = PipelineStateManager::new(db);

        let result = manager.update_job_status(1, "completed").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_state_manager_create_start_complete_cleanup() {
        use crate::entities::sync_jobs;
        use sea_orm::{DatabaseBackend, MockDatabase};

        let job_model = sync_jobs::Model {
            id: 11,
            tracking_id: 200,
            job_kind: "sync".to_string(),
            scheduled_at: Utc::now(),
            started_at: Some(Utc::now()),
            finished_at: None,
            status: "running".to_string(),
            error: None,
            attempt_count: 0,
            priority: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job_model.clone()]])
            .append_query_results::<sync_jobs::Model, _, _>(vec![vec![job_model.clone()]])
            .into_connection();
        let db = std::sync::Arc::new(db);
        let manager = PipelineStateManager::new(db);

        manager.create_state(11, 200).unwrap();
        manager.start_stage(11, PipelineStage::L2Snapshot).unwrap();
        manager
            .complete_stage(11, PipelineStage::L2Snapshot)
            .unwrap();
        let progress = manager.get_progress(11).await.unwrap();
        assert_eq!(progress.job_id, 11);
        assert_eq!(progress.tracking_id, 200);
        assert!(progress.progress_percent >= 0.0);
        manager.cleanup_state(11);
        let progress2 = manager.get_progress(11).await.unwrap();
        assert_eq!(progress2.job_id, 11);
    }
}
