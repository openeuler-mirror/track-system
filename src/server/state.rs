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

use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::collectors::{gitea::GiteaClient, gitee::GiteeClient};
use crate::scheduler::{SchedulerConfig, SchedulerManager};

/// 应用状态（共享给所有处理器）
#[derive(Clone)]
pub struct AppState {
    /// 数据库连接
    pub db: Arc<DatabaseConnection>,
    /// Gitee API 客户端（可选，测试场景可省略）
    pub gitee: Option<Arc<GiteeClient>>,
    /// Gitea API 客户端（可选，测试场景可省略）
    pub gitea: Option<Arc<GiteaClient>>,
    /// 调度器管理器（可选）
    pub scheduler_manager: Option<Arc<RwLock<SchedulerManager>>>,
}

impl AppState {
    /// 创建新的应用状态
    pub fn new(
        db: Arc<DatabaseConnection>,
        gitee: Option<GiteeClient>,
        gitea: Option<GiteaClient>,
    ) -> Self {
        Self {
            db,
