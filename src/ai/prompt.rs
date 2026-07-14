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
