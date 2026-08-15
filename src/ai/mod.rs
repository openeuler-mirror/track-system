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

//! AI assisted analysis for ecosystem and maintenance reports.
//!
//! The module is intentionally isolated from existing rule-based assessors. It
//! reads structured evidence already produced by ecosystem/maintenance modules
//! and produces advisory summaries that can be exposed through API handlers.

pub mod client;
pub mod config;
pub mod prompt;
pub mod service;
pub mod types;

pub use client::{AiClient, OpenAiCompatibleClient};
pub use config::AiConfig;
pub use service::AiAnalysisService;
pub use types::{
    AiAnalysisFinding, AiAnalysisRequest, AiAnalysisResponse, AiAnalysisSource, AiContext,
    AiRiskLevel,
};
