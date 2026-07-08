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

use axum::{
    routing::{delete, get, post, put},
    Router,
};

use crate::server::{handlers::package, state::AppState};

/// 创建软件包路由
pub fn package_routes() -> Router<AppState> {
    Router::new()
        .route("/packages", get(package::list_packages))
        .route("/packages", post(package::create_package))
        .route("/packages/:id", get(package::get_package))
        .route("/packages/:id", put(package::update_package))
        .route("/packages/:id", delete(package::delete_package))
        .route(
            "/packages/:id/tracking",
            get(package::get_package_with_tracking),
        )
}
