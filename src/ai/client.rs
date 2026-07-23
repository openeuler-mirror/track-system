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

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use super::config::AiConfig;

#[async_trait]
pub trait AiClient: Send + Sync {
    async fn analyze(&self, messages: Vec<Value>) -> Result<Value>;
    fn model(&self) -> &str;
}

pub struct OpenAiCompatibleClient {
    http: Client,
    config: AiConfig,
}

impl OpenAiCompatibleClient {
    pub fn new(config: AiConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(config.timeout)
            .build()
            .context("创建 AI HTTP 客户端失败")?;
        Ok(Self { http, config })
    }
}

#[async_trait]
impl AiClient for OpenAiCompatibleClient {
    async fn analyze(&self, messages: Vec<Value>) -> Result<Value> {
        let api_key = self
            .config
            .api_key
            .as_deref()
            .context("AI_API_KEY/OPENAI_API_KEY 未配置")?;
        let payload = json!({
            "model": self.config.model,
            "messages": messages,
            "temperature": 0.2,
            "response_format": {"type": "json_object"}
        });

        let response = self
            .http
            .post(self.config.chat_completions_url())
            .bearer_auth(api_key)
            .json(&payload)
            .send()
            .await
            .context("调用 AI 服务失败")?;

        let status = response.status();
        let body = response.text().await.context("读取 AI 响应失败")?;
        if !status.is_success() {
            anyhow::bail!("AI 服务返回错误 {}: {}", status, body);
        }

        let completion: ChatCompletionResponse =
            serde_json::from_str(&body).context("解析 AI 响应失败")?;
        let content = completion
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_deref())
            .context("AI 响应缺少 choices[0].message.content")?;

        serde_json::from_str(content).context("AI 响应不是合法 JSON")
    }

    fn model(&self) -> &str {
        &self.config.model
    }
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}
