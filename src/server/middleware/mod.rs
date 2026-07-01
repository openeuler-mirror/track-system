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

//! 服务器中间件模块

pub mod audit;
pub mod audit_service;
pub mod auth;
pub mod cors;

pub use audit::AuditMiddleware;
pub use audit_service::AuditService;
pub use auth::{
    auth_middleware, get_current_user, optional_auth_middleware, require_role, AuthConfig,
    AuthError, Claims, JwtTokenGenerator,
};
pub use cors::{
    create_cors_layer, create_cors_layer_from_env, create_permissive_cors_layer,
    create_strict_cors_layer, CorsConfig,
};
