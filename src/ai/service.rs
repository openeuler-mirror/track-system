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

use anyhow::Result;
use chrono::Utc;
use serde_json::Value;

use super::{
    client::{AiClient, OpenAiCompatibleClient},
    config::AiConfig,
    prompt::{build_messages, AiPromptOptions},
    types::{AiAnalysisFinding, AiAnalysisRequest, AiAnalysisResponse, AiContext, AiRiskLevel},
};

pub struct AiAnalysisService {
    config: AiConfig,
}

impl AiAnalysisService {
    pub fn from_env() -> Self {
        Self {
            config: AiConfig::from_env(),
        }
    }

    pub fn new(config: AiConfig) -> Self {
        Self { config }
    }

    pub async fn analyze(
        &self,
        context: AiContext,
        request: AiAnalysisRequest,
    ) -> Result<AiAnalysisResponse> {
        let language = request.language.as_deref().unwrap_or("中文");
        let max_chars = request
            .max_evidence_chars
            .unwrap_or(self.config.max_input_chars)
            .min(self.config.max_input_chars);

        if self.config.remote_available() {
            match self
                .analyze_remote(&context, &request, language, max_chars)
                .await
            {
                Ok(response) => return Ok(response),
                Err(err) => tracing::warn!(error = %err, "AI 远端分析失败，降级为本地启发式分析"),
            }
        }

        Ok(self.analyze_local(context))
    }

    async fn analyze_remote(
        &self,
        context: &AiContext,
        request: &AiAnalysisRequest,
        language: &str,
        max_chars: usize,
    ) -> Result<AiAnalysisResponse> {
        let client = OpenAiCompatibleClient::new(self.config.clone())?;
        let allow_external_research = request.allow_external_research.unwrap_or(true);
        let messages = build_messages(
            context,
