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
            findings.push(AiAnalysisFinding {
                title: "规则评估摘要".to_string(),
                risk,
                evidence: rule_summary.clone(),
                recommendation: "结合 evidence_summary 和 report_payload 核查关键证据来源是否完整。".to_string(),
            });
        }

        findings.extend(l0_security_quality_findings(&context, risk));

        if context.evidence.get("raw_evidence").is_none()
            && context.evidence.get("evidence_summary").is_none()
            && l0_community_assessment(&context).is_none()
        {
            findings.push(AiAnalysisFinding {
                title: "证据完整性不足".to_string(),
                risk: AiRiskLevel::Medium,
                evidence: "报告中未发现 raw_evidence/evidence_summary 字段。".to_string(),
                recommendation: "刷新生态或维护报告，确认采集器是否成功获取上游活跃度、版本、维护公告和仓库元数据。".to_string(),
            });
        }

        let recommended_actions = vec![
            "优先复核 high/critical 风险目标的原始证据和采集时间。".to_string(),
            "对证据缺失的目标重新执行 refresh，并检查外部平台 token、网络和限流配置。".to_string(),
            "将 AI 结论作为辅助建议，最终处置仍以规则评估、人工复核和审计记录为准。".to_string(),
        ];
        let sources_to_check = sources_to_check(&context);

        AiAnalysisResponse {
            source: context.source,
            generated_at: Utc::now(),
            model: "local-heuristic".to_string(),
            used_remote_model: false,
            external_research_used: false,
            summary,
            risk,
            confidence,
            findings,
            recommended_actions,
            external_references: Vec::new(),
            sources_to_check,
            raw_model_output: None,
        }
    }
}

fn sources_to_check(context: &AiContext) -> Vec<String> {
    let target = context
        .target_name
        .as_deref()
        .unwrap_or("目标组件")
        .to_string();
    let mut sources = Vec::new();

    if l0_community_assessment(context).is_none() {
        sources.push(format!(
            "{} L0 仓库 SECURITY.md、安全公告、security advisories 或 CVE 修复记录",
            target
        ));
        sources.push(format!(
            "{} L0 仓库 releases/tags 页面、发布说明、签名文件、checksum 或 provenance/attestation 资料",
            target
        ));
        sources.push(format!(
            "{} L0 仓库 CONTRIBUTING、CODEOWNERS、pull request review 规则或维护者文档",
            target
        ));
    }

    sources
}

fn l0_security_quality_findings(
    context: &AiContext,
    fallback_risk: AiRiskLevel,
) -> Vec<AiAnalysisFinding> {
    let Some(assessment) = l0_community_assessment(context) else {
        return Vec::new();
    };

    let mut findings = Vec::new();
    if let Some(section) = assessment.get("security") {
        findings.push(AiAnalysisFinding {
            title: "L0 社区安全评估".to_string(),
            risk: section_risk(section, fallback_risk),
            evidence: section_evidence_summary(
                section,
                &[
                    "has_security_policy",
                    "cve_fix_commits_last_12_months",
                    "cve_linked_issues_last_12_months",
                    "median_cve_fix_days",
                    "open_cve_backlog",
                ],
            ),
