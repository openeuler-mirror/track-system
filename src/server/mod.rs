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

pub mod api;
pub mod dto;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod routes;
pub mod state;

use axum::Router;
use sea_orm::DatabaseConnection;
use std::{env, net::SocketAddr, sync::Arc};

use crate::collectors::{gitea::GiteaClient, gitee::GiteeClient};

use self::{
    routes::{
        backport_routes, compare_routes, component_routes, ecosystem_routes, health_routes,
        metadata_routes, package_routes, reports_routes, sync_routes, tracking_routes,
    },
    state::AppState,
};

/// 创建 Axum 应用
pub fn create_app(db: Arc<DatabaseConnection>) -> Router {
    let gitee = env::var("GITEE_ACCESS_TOKEN")
        .ok()
        .and_then(|token| GiteeClient::new(token).ok());

    let gitea = match env::var("GITEA_ACCESS_TOKEN") {
        Ok(token) => {
            let base = env::var("GITEA_API_BASE")
                .unwrap_or_else(|_| "https://work.ctyun.cn/git/api/v1".to_string());
            GiteaClient::new(base, token).ok()
        }
        Err(_) => None,
    };

    let state = AppState::new(db, gitee, gitea);

    create_app_with_state(state)
}

/// 使用自定义 AppState 创建 Axum 应用
pub fn create_app_with_state(state: AppState) -> Router {
    Router::new()
        // API 路由
        .nest(
            "/api",
            health_routes()
                .merge(metadata_routes())
                .merge(compare_routes())
                .merge(reports_routes())
                .merge(package_routes())
                .merge(tracking_routes())
                .merge(sync_routes())
                .merge(backport_routes())
                .merge(component_routes())
                .merge(ecosystem_routes())
                .merge(crate::server::routes::snapshot_routes()),
        )
        .with_state(state)
}

/// 启动服务器
pub async fn serve(app: Router, addr: SocketAddr) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
