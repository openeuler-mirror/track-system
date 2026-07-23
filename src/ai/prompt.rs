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

use serde_json::json;

use super::types::AiContext;

#[derive(Debug, Clone, Copy)]
pub struct AiPromptOptions {
    pub allow_external_research: bool,
}

pub fn build_messages(
    context: &AiContext,
    question: Option<&str>,
    language: &str,
    max_evidence_chars: usize,
    options: AiPromptOptions,
) -> Vec<serde_json::Value> {
    let evidence = compact_json(&context.evidence, max_evidence_chars);
    let question = question.unwrap_or("请分析当前生态信息中的风险、证据充分性和建议动作。");
    let external_policy = if options.allow_external_research {
        "允许在输入证据不足时结合你可访问或已知的公开资料进行外部检索/公开信息判断，但必须区分 input_evidence、external_research、model_judgement，不能把外部判断伪装成系统已采集证据。"
    } else {
        "不允许使用外部检索或模型先验补充事实；只能基于输入证据分析。证据不足时必须说明缺口。"
    };

    vec![
        json!({
            "role": "system",
            "content": format!(
                "你是 CTyunOS 上游感知工具后端的智能分析模块。你需要优先基于输入证据分析，并在证据不足时给出可核验的补充判断。{} 输出必须是 JSON。",
                external_policy
            )
        }),
        json!({
            "role": "user",
            "content": format!(
                "请用{}分析以下生态/维护报告。\n\
                 要求：\n\
                 1. 输出 JSON 对象，字段为 summary、risk、confidence、findings、recommended_actions、external_research_used、external_references、sources_to_check。\n\
                 2. risk 只能是 low、medium、high、critical、unknown。\n\
                 3. findings 数组元素字段为 title、risk、evidence、recommendation，其中 evidence 必须说明来源类型：input_evidence、external_research 或 model_judgement。\n\
                 4. external_research_used 为布尔值；如果使用公开资料或外部检索判断，必须为 true，并在 external_references 中给出来源名称、URL 或可检索关键词。\n\
                 5. 证据不足时不要停止在“缺少证据”，应列出 sources_to_check，说明建议检索的官方仓库页面、安全公告、release 页面、review/贡献流程文档或签名校验资料。\n\
                 6. 如果证据中包含 L0 社区 security/quality 分区或 l0_community_assessment，必须单独评估 L0 社区安全和质量情况。\n\
                 7. L0 安全重点关注 has_security_policy、cve_fix_commits_last_12_months、cve_linked_issues_last_12_months、median_cve_fix_days、open_cve_backlog、是否定期发布 CVE 修复或安全公告。\n\
                 8. L0 质量重点关注 dedicated_code_reviewers、required_reviews、signed_releases、documented_release_artifact_signature、hash_verification_supported、provenance_attestation、release_checklist。\n\
                 9. 如果模型无法直接联网或无法确认外部信息，应明确写入 sources_to_check，而不是编造具体事实。\n\
                 10. 优先给出可执行的后端处置建议。\n\n\
                 分析问题：{}\n\n\
                 上下文：\n\
                 source={:?}\n\
                 target_name={:?}\n\
                 target_type={:?}\n\
                 platform={:?}\n\
                 report_type={:?}\n\
                 rule_risk={:?}\n\
                 rule_confidence={:?}\n\
                 rule_summary={:?}\n\n\
                 证据 JSON：\n{}",
                language,
                question,
                context.source,
                context.target_name,
                context.target_type,
                context.platform,
                context.report_type,
                context.rule_risk,
                context.rule_confidence,
                context.rule_summary,
                evidence
            )
        }),
    ]
}

fn compact_json(value: &serde_json::Value, max_chars: usize) -> String {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string());
    if text.chars().count() > max_chars {
        text = text.chars().take(max_chars).collect::<String>();
        text.push_str("\n...<truncated>");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::types::AiAnalysisSource;

    #[test]
    fn build_messages_contains_context() {
        let context = AiContext {
            source: AiAnalysisSource::AdHoc,
            target_name: Some("nginx".to_string()),
            target_type: Some("package".to_string()),
            platform: Some("github".to_string()),
            report_type: Some("test".to_string()),
            rule_risk: Some("medium".to_string()),
            rule_confidence: Some("high".to_string()),
            rule_summary: Some("summary".to_string()),
            evidence: serde_json::json!({"k":"v"}),
        };
        let messages = build_messages(
            &context,
            None,
            "中文",
            1000,
            AiPromptOptions {
                allow_external_research: true,
            },
        );
        assert_eq!(messages.len(), 2);
        assert!(messages[1]["content"].as_str().unwrap().contains("nginx"));
        assert!(messages[1]["content"]
            .as_str()
            .unwrap()
            .contains("L0 社区安全和质量"));
        assert!(messages[1]["content"]
            .as_str()
            .unwrap()
            .contains("external_research_used"));
        assert!(messages[0]["content"]
            .as_str()
            .unwrap()
            .contains("允许在输入证据不足时"));
    }

    #[test]
    fn build_messages_can_disable_external_research() {
        let context = AiContext {
            source: AiAnalysisSource::AdHoc,
            target_name: Some("nginx".to_string()),
            target_type: Some("package".to_string()),
            platform: Some("github".to_string()),
            report_type: Some("test".to_string()),
            rule_risk: Some("medium".to_string()),
            rule_confidence: Some("high".to_string()),
            rule_summary: Some("summary".to_string()),
            evidence: serde_json::json!({"k":"v"}),
        };
        let messages = build_messages(
            &context,
            None,
            "中文",
            1000,
            AiPromptOptions {
                allow_external_research: false,
            },
        );
        assert!(messages[0]["content"]
            .as_str()
            .unwrap()
            .contains("不允许使用外部检索"));
    }
}
