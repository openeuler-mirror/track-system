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
            gitee: gitee.map(Arc::new),
            gitea: gitea.map(Arc::new),
            scheduler_manager: None,
        }
    }

    /// 创建包含调度器的应用状态
    pub fn with_scheduler(
        db: DatabaseConnection,
        gitee: Option<GiteeClient>,
        gitea: Option<GiteaClient>,
        config: SchedulerConfig,
    ) -> Self {
        let db_arc = Arc::new(db);
        let (scheduler, _wake_rx) = SchedulerManager::new(db_arc.clone(), None, config);

        Self {
            db: db_arc,
            gitee: gitee.map(Arc::new),
            gitea: gitea.map(Arc::new),
            scheduler_manager: Some(Arc::new(RwLock::new(scheduler))),
        }
    }

    /// 创建仅包含数据库的状态（测试用）
    pub fn without_external_clients(db: DatabaseConnection) -> Self {
        Self {
            db: Arc::new(db),
            gitee: None,
            gitea: None,
            scheduler_manager: None,
        }
    }
}

impl AppState {
    pub fn scheduler(&self) -> crate::scheduler::SyncManager<'_> {
        crate::scheduler::SyncManager::new(self.db.as_ref())
    }
}
