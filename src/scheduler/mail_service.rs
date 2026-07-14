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

//! Mail delivery for generated scheduler artifacts.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use lettre::{
    message::{header::ContentType, Attachment, Mailbox, MultiPart},
    transport::smtp::{authentication::Credentials, client::Tls},
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};
use tracing::{debug, info};

use super::report_artifacts::{CveFixComparisonRow, ReportArtifact};

const DEFAULT_MAIL_SUBJECT: &str = "Track-System CVE/ISSUE对比报告";
const ENCRYPTED_PASSWORD_NONCE_LEN: usize = 12;
const DEFAULT_EMBED_XLSX_PREVIEW_ROWS: usize = 30;
const PREVIEW_DESCRIPTION_MAX_CHARS: usize = 220;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailConfig {
    pub enabled: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_tls: SmtpTlsMode,
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub timeout: Duration,
    pub embed_xlsx_preview: bool,
    pub embed_xlsx_preview_max_rows: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpTlsMode {
    StartTls,
    Wrapper,
    None,
}

impl MailConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("TRACK_MAIL_ENABLED", false),
            smtp_host: env_string("TRACK_MAIL_SMTP_HOST").unwrap_or_default(),
            smtp_port: env_u16("TRACK_MAIL_SMTP_PORT", 25),
            smtp_username: env_string("TRACK_MAIL_SMTP_USERNAME"),
            smtp_password: smtp_password_from_env(),
            smtp_tls: SmtpTlsMode::from_env_value(
                env_string("TRACK_MAIL_SMTP_TLS")
