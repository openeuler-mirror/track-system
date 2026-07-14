/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FITNESS FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

use std::env;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AiConfig {
    pub enabled: bool,
    pub provider: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout: Duration,
    pub max_input_chars: usize,
}

impl AiConfig {
    pub fn from_env() -> Self {
        let enabled = env_bool("AI_ANALYSIS_ENABLED", false);
        let provider = env::var("AI_PROVIDER").unwrap_or_else(|_| "openai-compatible".to_string());
        let base_url = env::var("AI_BASE_URL")
            .or_else(|_| env::var("OPENAI_BASE_URL"))
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let api_key = env::var("AI_API_KEY")
            .or_else(|_| env::var("OPENAI_API_KEY"))
            .ok()
            .filter(|value| !value.trim().is_empty());
        let model = env::var("AI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let timeout = Duration::from_secs(env_u64("AI_TIMEOUT_SECS", 30));
        let max_input_chars = env_usize("AI_MAX_INPUT_CHARS", 16_000);

        Self {
            enabled,
