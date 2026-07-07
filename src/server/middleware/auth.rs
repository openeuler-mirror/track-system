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

//! JWT 认证中间件
//!
//! 提供基于 JWT (JSON Web Token) 的认证功能

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin, sync::Arc};

/// JWT Claims 结构
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// 用户 ID
    pub sub: String,
    /// 用户名
    pub username: String,
    /// 用户角色
    pub role: String,
    /// 过期时间（Unix 时间戳）
    pub exp: i64,
    /// 签发时间（Unix 时间戳）
    pub iat: i64,
}

impl Claims {
    /// 创建新的 Claims
    pub fn new(user_id: String, username: String, role: String, expiry_hours: i64) -> Self {
        let now = Utc::now();
        let exp = now + Duration::hours(expiry_hours);

        Self {
            sub: user_id,
            username,
            role,
            exp: exp.timestamp(),
            iat: now.timestamp(),
        }
    }

    /// 检查 token 是否过期
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp();
        self.exp < now
    }
}

/// JWT 认证配置
#[derive(Clone)]
pub struct AuthConfig {
    /// JWT 密钥
    pub secret: String,
    /// Token 过期时间（小时）
    pub expiry_hours: i64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            secret: "default-secret-key-change-in-production".to_string(),
            expiry_hours: 24,
        }
    }
}

impl AuthConfig {
    /// 创建新的认证配置
    pub fn new(secret: String, expiry_hours: i64) -> Self {
        Self {
            secret,
            expiry_hours,
        }
    }

    /// 从环境变量加载配置
    pub fn from_env() -> Self {
        let secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "default-secret-key-change-in-production".to_string());
        let expiry_hours = std::env::var("JWT_EXPIRY_HOURS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(24);

        Self {
            secret,
            expiry_hours,
        }
    }
}

/// JWT Token 生成器
pub struct JwtTokenGenerator {
    config: AuthConfig,
}

impl JwtTokenGenerator {
    /// 创建新的 Token 生成器
    pub fn new(config: AuthConfig) -> Self {
        Self { config }
    }

    /// 生成 JWT Token
    pub fn generate_token(
        &self,
        user_id: String,
        username: String,
        role: String,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let claims = Claims::new(user_id, username, role, self.config.expiry_hours);

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.secret.as_bytes()),
        )
    }

    /// 验证 JWT Token
    pub fn verify_token(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.config.secret.as_bytes()),
            &Validation::default(),
        )?;

        Ok(token_data.claims)
    }

    /// 刷新 Token
    pub fn refresh_token(&self, old_token: &str) -> Result<String, jsonwebtoken::errors::Error> {
        let claims = self.verify_token(old_token)?;

        // 生成新的 token
        self.generate_token(claims.sub, claims.username, claims.role)
    }
}

/// 认证错误
#[derive(Debug)]
pub enum AuthError {
    /// Token 缺失
    MissingToken,
    /// Token 无效
    InvalidToken,
    /// Token 过期
    ExpiredToken,
    /// 权限不足
    InsufficientPermissions,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "Missing authentication token"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid authentication token"),
            AuthError::ExpiredToken => (StatusCode::UNAUTHORIZED, "Token has expired"),
            AuthError::InsufficientPermissions => {
                (StatusCode::FORBIDDEN, "Insufficient permissions")
            }
        };

        let body = serde_json::json!({
            "success": false,
            "error": "AuthenticationError",
            "message": message,
        });

        (status, axum::Json(body)).into_response()
    }
}

/// 从请求头中提取 Bearer Token
fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Bearer ").map(|s| s.to_string()))
}

/// JWT 认证中间件
///
/// 验证请求中的 JWT Token，并将 Claims 添加到请求扩展中
pub async fn auth_middleware(
    State(config): State<Arc<AuthConfig>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // 提取 token
    let token = extract_bearer_token(request.headers()).ok_or(AuthError::MissingToken)?;

    // 验证 token
    let generator = JwtTokenGenerator::new((*config).clone());
    let claims = generator
        .verify_token(&token)
        .map_err(|_| AuthError::InvalidToken)?;

    // 检查是否过期
    if claims.is_expired() {
        return Err(AuthError::ExpiredToken);
    }

    // 将 claims 添加到请求扩展中，供后续处理器使用
    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}

/// 可选的认证中间件
///
