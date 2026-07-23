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
            request.question.as_deref(),
            language,
            max_chars,
            AiPromptOptions {
                allow_external_research,
            },
        );
        let raw = client.analyze(messages).await?;

        Ok(AiAnalysisResponse {
            source: context.source,
            generated_at: Utc::now(),
            model: client.model().to_string(),
            used_remote_model: true,
            external_research_used: bool_field(&raw, "external_research_used"),
            summary: string_field(&raw, "summary")
                .unwrap_or_else(|| "AI 模型未返回 summary 字段".to_string()),
            risk: string_field(&raw, "risk")
                .map(|value| AiRiskLevel::from_report_value(&value))
                .unwrap_or_else(|| infer_risk_from_context(context)),
            confidence: string_field(&raw, "confidence").unwrap_or_else(|| "medium".to_string()),
            findings: parse_findings(&raw),
            recommended_actions: parse_string_array(&raw, "recommended_actions"),
            external_references: parse_string_array(&raw, "external_references"),
            sources_to_check: parse_string_array(&raw, "sources_to_check"),
            raw_model_output: Some(raw),
        })
    }

    fn analyze_local(&self, context: AiContext) -> AiAnalysisResponse {
        let risk = infer_risk_from_context(&context);
        let confidence = context
            .rule_confidence
            .clone()
            .unwrap_or_else(|| "medium".to_string());
        let target_name = context
            .target_name
            .clone()
            .unwrap_or_else(|| "未知目标".to_string());
        let summary = format!(
            "本地启发式分析基于现有规则报告生成：{} 当前规则风险为 {}，规则置信度为 {}。未启用远端 AI 模型时，建议以规则评估结果和证据完整性作为处置依据。",
            target_name,
            context.rule_risk.as_deref().unwrap_or("unknown"),
            confidence
        );

        let mut findings = Vec::new();
        if let Some(rule_summary) = &context.rule_summary {
