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
                    .as_deref()
                    .unwrap_or("starttls"),
            ),
            from: env_string("TRACK_MAIL_FROM").unwrap_or_default(),
            to: env_list("TRACK_MAIL_TO"),
            cc: env_list("TRACK_MAIL_CC"),
            subject: env_string("TRACK_MAIL_SUBJECT")
                .unwrap_or_else(|| DEFAULT_MAIL_SUBJECT.to_string()),
            timeout: Duration::from_secs(env_u64("TRACK_MAIL_TIMEOUT_SECS", 60)),
            embed_xlsx_preview: env_bool("TRACK_MAIL_EMBED_XLSX_PREVIEW", true),
            embed_xlsx_preview_max_rows: env_usize(
                "TRACK_MAIL_EMBED_XLSX_MAX_ROWS",
                DEFAULT_EMBED_XLSX_PREVIEW_ROWS,
            ),
        }
    }

    pub fn validate_enabled(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        if self.smtp_host.trim().is_empty() {
            anyhow::bail!("TRACK_MAIL_SMTP_HOST 不能为空");
        }
        if self.from.trim().is_empty() {
            anyhow::bail!("TRACK_MAIL_FROM 不能为空");
        }
        if self.to.is_empty() {
            anyhow::bail!("TRACK_MAIL_TO 不能为空");
        }
        Ok(())
    }

    fn has_auth(&self) -> bool {
        self.smtp_username.is_some() && self.smtp_password.is_some()
    }
}

impl SmtpTlsMode {
    fn from_env_value(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" | "off" | "false" | "0" => Self::None,
            "wrapper" | "smtps" | "tls" => Self::Wrapper,
            _ => Self::StartTls,
        }
    }
}

pub struct MailService {
    config: MailConfig,
}

impl MailService {
    pub fn from_env() -> Self {
        Self {
            config: MailConfig::from_env(),
        }
    }

    pub fn new(config: MailConfig) -> Self {
        Self { config }
    }

    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    pub async fn send_xlsx_artifact(&self, artifact: &ReportArtifact) -> Result<()> {
        if !self.config.enabled {
            debug!("xlsx 邮件发送未启用");
            return Ok(());
        }
        self.config.validate_enabled()?;
        if artifact.rows == 0 {
            debug!(path = %artifact.path, "xlsx 内容为空，跳过邮件发送");
            return Ok(());
        }

        let attachment_path = PathBuf::from(&artifact.path);
        let message = build_artifact_message(&self.config, artifact, &attachment_path)?;
        let transport = build_transport(&self.config)?;

        tokio::time::timeout(self.config.timeout, transport.send(message))
            .await
            .context("发送 xlsx 邮件超时")?
            .context("发送 xlsx 邮件失败")?;

        info!(
            path = %artifact.path,
            rows = artifact.rows,
            recipients = self.config.to.join(","),
            "xlsx 邮件发送成功"
        );
