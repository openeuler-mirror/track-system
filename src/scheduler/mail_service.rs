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

use anyhow::{Context, Result};
use lettre::{
    message::{header::ContentType, Attachment, Mailbox, MultiPart},
    transport::smtp::{authentication::Credentials, client::Tls},
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use std::{env, path::PathBuf, time::Duration};
use tracing::{debug, info};

use crate::utils::secret::decrypt_secret_from_env;

use super::report_artifacts::{CveFixComparisonRow, ReportArtifact};

const DEFAULT_MAIL_SUBJECT: &str = "Track-System CVE/ISSUE对比报告";
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

        Ok(())
    }
}

fn build_transport(config: &MailConfig) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    let mut builder = match config.smtp_tls {
        SmtpTlsMode::Wrapper => {
            AsyncSmtpTransport::<Tokio1Executor>::relay(config.smtp_host.trim())
                .context("创建 SMTPS 传输失败")?
        }
        SmtpTlsMode::StartTls => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(config.smtp_host.trim())
                .context("创建 STARTTLS SMTP 传输失败")?
        }
        SmtpTlsMode::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(
            config.smtp_host.trim().to_string(),
        )
        .tls(Tls::None),
    }
    .port(config.smtp_port);

    if config.has_auth() {
        builder = builder.credentials(Credentials::new(
            config.smtp_username.clone().unwrap_or_default(),
            config.smtp_password.clone().unwrap_or_default(),
        ));
    }

    Ok(builder.build())
}

fn build_artifact_message(
    config: &MailConfig,
    artifact: &ReportArtifact,
    attachment_path: &Path,
) -> Result<Message> {
    let file_name = attachment_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("track-system-cve-fix-comparison.xlsx")
        .to_string();
    let attachment_bytes = fs::read(attachment_path)
        .with_context(|| format!("读取 xlsx 附件失败: {}", attachment_path.display()))?;

    let mut builder = Message::builder()
        .from(parse_mailbox(&config.from).context("解析 TRACK_MAIL_FROM 失败")?)
        .subject(config.subject.clone());

    for recipient in &config.to {
        builder = builder.to(parse_mailbox(recipient)
            .with_context(|| format!("解析 TRACK_MAIL_TO 收件人失败: {}", recipient))?);
    }
    for recipient in &config.cc {
        builder = builder.cc(parse_mailbox(recipient)
            .with_context(|| format!("解析 TRACK_MAIL_CC 收件人失败: {}", recipient))?);
    }

    let plain_body = build_plain_body(config, artifact);
    let html_body = build_html_body(config, artifact);
    let content_type =
        ContentType::parse("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
            .context("解析 xlsx Content-Type 失败")?;
    let message = builder
        .multipart(
            MultiPart::mixed()
                .multipart(MultiPart::alternative_plain_html(plain_body, html_body))
                .singlepart(Attachment::new(file_name).body(attachment_bytes, content_type)),
        )
        .context("构造 xlsx 邮件失败")?;

    Ok(message)
}

fn build_plain_body(config: &MailConfig, artifact: &ReportArtifact) -> String {
    let mut body = format!(
        "Track-System 已生成本轮 CVE/ISSUE 对比 xlsx 报告。\n\n生成时间: {}",
        artifact.generated_at
    );

    if config.embed_xlsx_preview {
        let preview_rows = artifact
            .preview_rows
            .iter()
            .take(config.embed_xlsx_preview_max_rows)
            .collect::<Vec<_>>();
        if preview_rows.is_empty() {
            body.push_str("\n\n正文预览: 无可展示数据，完整内容请查看附件。");
        } else {
            body.push_str("\n\n正文预览:");
            let mut current_system_version = "";
            for row in preview_rows {
                if row.system_version != current_system_version {
                    current_system_version = &row.system_version;
                    body.push_str(&format!("\n\n[{}]", current_system_version));
                }
                body.push_str(&format!(
                    "\n- {} | {} | 上游修复版本: {} | 当前版本: {} | {}",
                    row.package,
                    row.cve_or_issue,
                    row.upstream_fixed_version,
                    row.ctyunos_current_version,
                    truncate_text(
                        &row.description.replace('\n', " "),
                        PREVIEW_DESCRIPTION_MAX_CHARS
                    )
                ));
            }
            if artifact.preview_rows.len() > config.embed_xlsx_preview_max_rows {
                body.push_str(&format!(
                    "\n\n仅展示前 {} 行，完整内容请查看附件。",
                    config.embed_xlsx_preview_max_rows
                ));
            }
        }
    }

    body.push_str("\n\n该邮件由系统自动发送。");
    body
}

fn build_html_body(config: &MailConfig, artifact: &ReportArtifact) -> String {
    let mut html = format!(
        r#"<!doctype html><html><body style="font-family:Arial,'Microsoft YaHei',sans-serif;font-size:14px;color:#1f2937;">
<p>Track-System 已生成本轮 CVE/ISSUE 对比 xlsx 报告。</p>
<table cellpadding="4" cellspacing="0" style="border-collapse:collapse;margin:12px 0;">
<tr><td style="color:#6b7280;">生成时间</td><td>{}</td></tr>
</table>"#,
        html_escape(&artifact.generated_at)
    );

    if config.embed_xlsx_preview {
        let preview_rows = artifact
            .preview_rows
            .iter()
            .take(config.embed_xlsx_preview_max_rows)
            .collect::<Vec<_>>();
        if preview_rows.is_empty() {
            html.push_str("<p>正文预览：无可展示数据，完整内容请查看附件。</p>");
        } else {
            html.push_str("<p>正文预览：</p>");
            append_preview_tables_html(&mut html, &preview_rows);
            if artifact.preview_rows.len() > config.embed_xlsx_preview_max_rows {
                html.push_str(&format!(
                    "<p>仅展示前 {} 行，完整内容请查看附件。</p>",
                    config.embed_xlsx_preview_max_rows
                ));
            }
        }
    }

    html.push_str("<p style=\"color:#6b7280;\">该邮件由系统自动发送。</p></body></html>");
    html
}

fn append_preview_tables_html(html: &mut String, rows: &[&CveFixComparisonRow]) {
    let mut current_system_version = "";
    let mut table_open = false;
    for row in rows {
        if row.system_version != current_system_version {
            if table_open {
                html.push_str("</tbody></table>");
            }
            current_system_version = &row.system_version;
            html.push_str(&format!(
                "<h3 style=\"font-size:15px;margin:16px 0 8px;\">{}</h3>",
                html_escape(current_system_version)
            ));
            html.push_str(
                r#"<table cellpadding="6" cellspacing="0" style="border-collapse:collapse;width:100%;max-width:1200px;">
<thead><tr style="background:#f3f4f6;">
<th style="border:1px solid #d1d5db;text-align:left;">软件包</th>
<th style="border:1px solid #d1d5db;text-align:left;">CVE/ISSUE编号</th>
<th style="border:1px solid #d1d5db;text-align:left;">上游修复版本</th>
<th style="border:1px solid #d1d5db;text-align:left;">CTyunOS当前版本</th>
<th style="border:1px solid #d1d5db;text-align:left;">描述摘要</th>
</tr></thead><tbody>"#,
            );
            table_open = true;
        }

        html.push_str(&format!(
            r#"<tr>
<td style="border:1px solid #d1d5db;vertical-align:top;">{}</td>
<td style="border:1px solid #d1d5db;vertical-align:top;">{}</td>
<td style="border:1px solid #d1d5db;vertical-align:top;">{}</td>
<td style="border:1px solid #d1d5db;vertical-align:top;">{}</td>
<td style="border:1px solid #d1d5db;vertical-align:top;">{}</td>
</tr>"#,
            html_escape(&row.package),
            html_escape(&row.cve_or_issue),
            html_escape(&row.upstream_fixed_version),
            html_escape(&row.ctyunos_current_version),
            html_escape(&truncate_text(
                &row.description.replace('\n', " "),
                PREVIEW_DESCRIPTION_MAX_CHARS
            )),
        ));
    }
    if table_open {
        html.push_str("</tbody></table>");
    }
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn parse_mailbox(value: &str) -> Result<Mailbox> {
    value
        .trim()
        .parse::<Mailbox>()
        .with_context(|| format!("无效邮箱地址: {}", value))
}

fn smtp_password_from_env() -> Option<String> {
    match (
        env_string("TRACK_MAIL_SMTP_PASSWORD_ENCRYPTED"),
        env_string("TRACK_MAIL_SMTP_PASSWORD_KEY_FILE"),
    ) {
        (Some(_), Some(_)) => {
            match decrypt_secret_from_env(
                "TRACK_MAIL_SMTP_PASSWORD_ENCRYPTED",
                "TRACK_MAIL_SMTP_PASSWORD_KEY_FILE",
            ) {
                Ok(password) => Some(password),
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "解密 TRACK_MAIL_SMTP_PASSWORD_ENCRYPTED 失败，将回退到明文兼容配置"
                    );
                    env_string("TRACK_MAIL_SMTP_PASSWORD")
                }
            }
        }
        _ => env_string("TRACK_MAIL_SMTP_PASSWORD"),
    }
}

fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

fn env_string(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_list(key: &str) -> Vec<String> {
    env::var(key)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn env_u16(key: &str, default: u16) -> u16 {
    env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::{
        aead::{Aead, OsRng},
        AeadCore, Aes256Gcm, KeyInit,
    };
    use base64::{engine::general_purpose, Engine as _};
    use serial_test::serial;
    use std::{
        fs,
        sync::{Mutex, OnceLock},
    };
    use tempfile::tempdir;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    #[serial]
    fn config_from_env_defaults_to_disabled() {
        let _guard = env_lock().lock().unwrap();
        let _enabled = EnvVarGuard::remove("TRACK_MAIL_ENABLED");
        let config = MailConfig::from_env();

        assert!(!config.enabled);
        assert_eq!(config.smtp_port, 25);
        assert_eq!(config.smtp_tls, SmtpTlsMode::StartTls);
    }

    #[test]
    #[serial]
    fn config_from_env_parses_recipients_and_tls() {
        let _guard = env_lock().lock().unwrap();
        let _enabled = EnvVarGuard::set("TRACK_MAIL_ENABLED", "true");
        let _host = EnvVarGuard::set("TRACK_MAIL_SMTP_HOST", "smtp.example.com");
        let _port = EnvVarGuard::set("TRACK_MAIL_SMTP_PORT", "465");
        let _tls = EnvVarGuard::set("TRACK_MAIL_SMTP_TLS", "wrapper");
        let _from = EnvVarGuard::set("TRACK_MAIL_FROM", "track@example.com");
        let _to = EnvVarGuard::set("TRACK_MAIL_TO", "a@example.com, b@example.com");
        let _cc = EnvVarGuard::set("TRACK_MAIL_CC", "c@example.com");
        let _password = EnvVarGuard::remove("TRACK_MAIL_SMTP_PASSWORD");
        let _password_encrypted = EnvVarGuard::remove("TRACK_MAIL_SMTP_PASSWORD_ENCRYPTED");
        let _password_key = EnvVarGuard::remove("TRACK_MAIL_SMTP_PASSWORD_KEY_FILE");

        let config = MailConfig::from_env();

        assert!(config.enabled);
        assert_eq!(config.smtp_host, "smtp.example.com");
        assert_eq!(config.smtp_port, 465);
        assert_eq!(config.smtp_tls, SmtpTlsMode::Wrapper);
        assert_eq!(config.to, vec!["a@example.com", "b@example.com"]);
        assert_eq!(config.cc, vec!["c@example.com"]);
        config.validate_enabled().unwrap();
    }

    #[test]
    #[serial]
    fn config_from_env_decrypts_smtp_password() {
        let _guard = env_lock().lock().unwrap();
        let dir = tempdir().unwrap();
        let key_path = dir.path().join("smtp-password.key");
        let key = [7_u8; 32];
        fs::write(
            &key_path,
            format!("base64:{}", general_purpose::STANDARD.encode(key)),
        )
        .unwrap();
        let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, "secret-password".as_bytes())
            .unwrap();
        let mut payload = nonce.to_vec();
        payload.extend(ciphertext);
        let encrypted = general_purpose::STANDARD.encode(payload);

        let _enabled = EnvVarGuard::set("TRACK_MAIL_ENABLED", "true");
        let _host = EnvVarGuard::set("TRACK_MAIL_SMTP_HOST", "smtp.example.com");
        let _from = EnvVarGuard::set("TRACK_MAIL_FROM", "track@example.com");
        let _to = EnvVarGuard::set("TRACK_MAIL_TO", "a@example.com");
        let _username = EnvVarGuard::set("TRACK_MAIL_SMTP_USERNAME", "track@example.com");
        let _password = EnvVarGuard::remove("TRACK_MAIL_SMTP_PASSWORD");
        let _encrypted = EnvVarGuard::set("TRACK_MAIL_SMTP_PASSWORD_ENCRYPTED", &encrypted);
        let _key_file = EnvVarGuard::set(
            "TRACK_MAIL_SMTP_PASSWORD_KEY_FILE",
            key_path.to_str().unwrap(),
        );

        let config = MailConfig::from_env();

        assert_eq!(config.smtp_password.as_deref(), Some("secret-password"));
        assert!(config.has_auth());
    }

    #[test]
    fn build_message_attaches_xlsx_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("report.xlsx");
        fs::write(&path, b"xlsx-bytes").unwrap();
        let config = MailConfig {
            enabled: true,
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 25,
            smtp_username: None,
            smtp_password: None,
            smtp_tls: SmtpTlsMode::None,
            from: "track@example.com".to_string(),
            to: vec!["ops@example.com".to_string()],
            cc: Vec::new(),
            subject: "subject".to_string(),
            timeout: Duration::from_secs(1),
            embed_xlsx_preview: true,
            embed_xlsx_preview_max_rows: 30,
        };
        let artifact = ReportArtifact {
            artifact_type: "cve_fix_comparison_xlsx".to_string(),
            path: path.to_string_lossy().to_string(),
            format: "xlsx".to_string(),
            rows: 3,
            source: "scheduler_round".to_string(),
            template: "template".to_string(),
            generated_at: "2026-06-02T00:00:00Z".to_string(),
            preview_rows: vec![CveFixComparisonRow {
                package: "bash".to_string(),
                cve_or_issue: "CVE-2026-1234".to_string(),
                upstream_fixed_version: "5.2-3".to_string(),
                ctyunos_current_version: "5.2-1".to_string(),
                system_version: "CTyunOS22.06".to_string(),
                description: "Fix CVE-2026-1234".to_string(),
                xingkong_ticket_no: "97883".to_string(),
                commit_url: Some("https://example.com/commit/abc".to_string()),
            }],
        };

        let message = build_artifact_message(&config, &artifact, &path).unwrap();
        let formatted = String::from_utf8(message.formatted()).unwrap();

        assert!(formatted.contains("ops@example.com"));
        assert!(formatted.contains("report.xlsx"));
        assert!(formatted.contains("Content-Type: text/html"));
        assert!(formatted.contains("CTyunOS22.06"));
        assert!(formatted.contains("CVE-2026-1234"));
        assert!(
            formatted.contains("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        );
    }

    #[test]
    fn build_body_limits_embedded_xlsx_preview_rows() {
        let config = MailConfig {
            enabled: true,
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 25,
            smtp_username: None,
            smtp_password: None,
            smtp_tls: SmtpTlsMode::None,
            from: "track@example.com".to_string(),
            to: vec!["ops@example.com".to_string()],
            cc: Vec::new(),
            subject: "subject".to_string(),
            timeout: Duration::from_secs(1),
            embed_xlsx_preview: true,
            embed_xlsx_preview_max_rows: 1,
        };
        let artifact = ReportArtifact {
            artifact_type: "cve_fix_comparison_xlsx".to_string(),
            path: "/tmp/report.xlsx".to_string(),
            format: "xlsx".to_string(),
            rows: 2,
            source: "scheduler_round".to_string(),
            template: "template".to_string(),
            generated_at: "2026-06-02T00:00:00Z".to_string(),
            preview_rows: vec![
                CveFixComparisonRow {
                    package: "bash".to_string(),
                    cve_or_issue: "ISSUE-10200".to_string(),
                    upstream_fixed_version: "5.2-3".to_string(),
                    ctyunos_current_version: "5.2-1".to_string(),
                    system_version: "CTyunOS22.06".to_string(),
                    description: "Fix bash issue".to_string(),
                    xingkong_ticket_no: "97883".to_string(),
                    commit_url: None,
                },
                CveFixComparisonRow {
                    package: "coreutils".to_string(),
                    cve_or_issue: "ISSUE-10201".to_string(),
                    upstream_fixed_version: "9.5-4".to_string(),
                    ctyunos_current_version: "9.5-2".to_string(),
                    system_version: "CTyunOS25.07".to_string(),
                    description: "Fix coreutils issue".to_string(),
                    xingkong_ticket_no: "97883".to_string(),
                    commit_url: None,
                },
            ],
        };

        let plain = build_plain_body(&config, &artifact);
        let html = build_html_body(&config, &artifact);

        assert!(plain.contains("bash"));
        assert!(!plain.contains("coreutils"));
        assert!(plain.contains("仅展示前 1 行"));
        assert!(html.contains("bash"));
        assert!(!html.contains("coreutils"));
        assert!(html.contains("仅展示前 1 行"));
    }

    struct EnvVarGuard {
        key: &'static str,
        old_value: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let old_value = env::var(key).ok();
            env::set_var(key, value);
            Self { key, old_value }
        }

        fn remove(key: &'static str) -> Self {
            let old_value = env::var(key).ok();
            env::remove_var(key);
            Self { key, old_value }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.old_value {
                env::set_var(self.key, value);
            } else {
                env::remove_var(self.key);
            }
        }
    }
}
