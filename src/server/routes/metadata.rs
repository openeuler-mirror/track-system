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

//! 元数据管理路由

use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::server::{handlers::metadata, state::AppState};

/// 创建元数据管理路由
///
/// 提供完整的 CRUD 操作，符合 RESTful 规范
pub fn metadata_routes() -> Router<AppState> {
    Router::new()
        // L0 元数据管理
        .route("/metadata/l0", post(metadata::import_l0_metadata))
        .route("/metadata/l0", get(metadata::list_l0_metadata))
        .route("/metadata/l0/:id", get(metadata::get_l0_metadata))
        .route("/metadata/l0/:id", delete(metadata::delete_l0_metadata))
        // L1 元数据管理
        .route("/metadata/l1", post(metadata::import_l1_metadata))
        .route("/metadata/l1", get(metadata::list_l1_metadata))
        .route("/metadata/l1/:id", get(metadata::get_l1_metadata))
        .route("/metadata/l1/:id", delete(metadata::delete_l1_metadata))
        // L2 元数据管理
        .route("/metadata/l2", post(metadata::import_l2_metadata))
        .route("/metadata/l2", get(metadata::list_l2_metadata))
        .route("/metadata/l2/:id", get(metadata::get_l2_metadata))
        .route("/metadata/l2/:id", delete(metadata::delete_l2_metadata))
}
