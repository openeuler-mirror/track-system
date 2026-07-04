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

//! 任务调度器模块
//!
//! 负责根据软件等级和同步间隔管理同步任务

pub mod cron_scheduler;
pub mod pipeline_executor;
pub mod pipeline_stages;
pub mod pipeline_state;
pub mod scheduler_manager;
pub mod sync_executor;
pub mod sync_manager;
pub mod sync_service;

pub use cron_scheduler::CronScheduler;
pub use pipeline_executor::{
    BackportSuggestionResult, ClassificationResult, DiffComparisonResult, JobProgress,
    L1IngestionResult, L2SnapshotResult, PipelineExecutor, PipelineStage, ReportGenerationResult,
    StageResult, SyncJobResult,
};
pub use pipeline_state::{PipelineState, PipelineStateManager};
pub use scheduler_manager::{SchedulerConfig, SchedulerManager, SchedulerStatus};
pub use sync_executor::{SyncApiClient, SyncExecutionStats, SyncExecutor};
pub use sync_manager::SyncManager;
pub use sync_service::{SyncResult, SyncService, SyncStatus};
