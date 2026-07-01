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

//! CORS 中间件
//!
//! 提供跨域资源共享（CORS）配置

use axum::http::{header, HeaderName, HeaderValue, Method};
use tower_http::cors::{Any, CorsLayer};

/// CORS 配置
#[derive(Clone, Debug)]
pub struct CorsConfig {
    /// 允许的来源
    pub allowed_origins: Vec<String>,
    /// 允许的方法
    pub allowed_methods: Vec<Method>,
    /// 允许的头
    pub allowed_headers: Vec<String>,
    /// 是否允许凭证
    pub allow_credentials: bool,
    /// 预检请求缓存时间（秒）
    pub max_age: u64,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec![
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
                Method::PATCH,
            ],
            allowed_headers: vec![
                header::CONTENT_TYPE.to_string(),
                header::AUTHORIZATION.to_string(),
                header::ACCEPT.to_string(),
                "x-requested-with".to_string(),
            ],
            allow_credentials: true,
            max_age: 3600,
        }
    }
}

impl CorsConfig {
    /// 创建新的 CORS 配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置允许的来源
    pub fn with_origins(mut self, origins: Vec<String>) -> Self {
        self.allowed_origins = origins;
        self
    }

    /// 设置允许的方法
    pub fn with_methods(mut self, methods: Vec<Method>) -> Self {
        self.allowed_methods = methods;
        self
    }

    /// 设置允许的头
    pub fn with_headers(mut self, headers: Vec<String>) -> Self {
        self.allowed_headers = headers;
        self
    }

    /// 设置是否允许凭证
    pub fn with_credentials(mut self, allow: bool) -> Self {
        self.allow_credentials = allow;
        self
    }

    /// 设置预检请求缓存时间
    pub fn with_max_age(mut self, seconds: u64) -> Self {
        self.max_age = seconds;
        self
    }

    /// 从环境变量加载配置
    pub fn from_env() -> Self {
        let allowed_origins = std::env::var("CORS_ALLOWED_ORIGINS")
            .ok()
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_else(|| vec!["*".to_string()]);

        let allow_credentials = std::env::var("CORS_ALLOW_CREDENTIALS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(true);

        let max_age = std::env::var("CORS_MAX_AGE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3600);

        Self {
            allowed_origins,
            allow_credentials,
            max_age,
            ..Default::default()
        }
    }

    /// 构建 CorsLayer
    pub fn build(&self) -> CorsLayer {
        let mut cors = CorsLayer::new();

        // 配置允许的来源
        if self.allowed_origins.contains(&"*".to_string()) {
            cors = cors.allow_origin(Any);
        } else {
            let origins: Vec<HeaderValue> = self
                .allowed_origins
                .iter()
                .filter_map(|origin| origin.parse().ok())
                .collect();
            cors = cors.allow_origin(origins);
        }

        // 配置允许的方法
        cors = cors.allow_methods(self.allowed_methods.clone());

        // 配置允许的头
