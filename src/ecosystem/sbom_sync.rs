use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

use crate::entities::{ecosystem_reports, ecosystem_targets};

#[derive(Debug, Clone)]
pub struct SbomCommunitySyncConfig {
    pub endpoint_url: String,
    pub inner_secret: String,
    pub timeout: Duration,
    pub status: Option<String>,
}

impl SbomCommunitySyncConfig {
    pub fn from_env() -> Result<Option<Self>> {
        if !env_bool("SBOM_COMMUNITY_SYNC_ENABLED", false) {
            return Ok(None);
        }

        let endpoint_url = required_env("SBOM_COMMUNITY_SYNC_URL")?;
        let inner_secret = required_env("SBOM_COMMUNITY_INNER_SECRET")?;
        let timeout_secs = std::env::var("SBOM_COMMUNITY_SYNC_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(5);
        let status = optional_env("SBOM_COMMUNITY_SYNC_STATUS");

        Ok(Some(Self {
            endpoint_url,
            inner_secret,
            timeout: Duration::from_secs(timeout_secs),
            status,
        }))
    }
}

#[derive(Debug, Clone)]
pub struct SbomCommunitySyncClient {
    client: Client,
    config: SbomCommunitySyncConfig,
}

impl SbomCommunitySyncClient {
    pub fn new(config: SbomCommunitySyncConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .context("create SBOM community sync HTTP client failed")?;
        Ok(Self { client, config })
    }

    pub fn from_env() -> Result<Option<Self>> {
        SbomCommunitySyncConfig::from_env()?
            .map(Self::new)
            .transpose()
    }

    pub async fn sync_report(
        &self,
        target: &ecosystem_targets::Model,
        report: &ecosystem_reports::Model,
    ) -> Result<SbomCommunitySyncResponse> {
        let request = build_community_inner_sync_request(target, report, &self.config);
        let response = self
            .client
            .post(&self.config.endpoint_url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("send SBOM community sync request failed")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("read SBOM community sync response failed")?;

        if !status.is_success() {
            return Err(anyhow!(
                "SBOM community sync HTTP status {}: {}",
                status,
                body
            ));
        }

        parse_community_inner_sync_response(&body)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommunityInnerSyncReq {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub necessity_introduction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub introduction_department: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_structure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foundation_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_lifecycle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cla_info: Option<String>,
    pub inner_secret: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SbomCommunitySyncResponse {
    pub code: i32,
    #[serde(default)]
    pub msg: String,
    pub data: Option<SbomCommunitySyncData>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SbomCommunitySyncData {
    pub community_id: Option<String>,
    pub name: Option<String>,
    pub action: Option<String>,
}

pub fn build_community_inner_sync_request(
    target: &ecosystem_targets::Model,
    report: &ecosystem_reports::Model,
    config: &SbomCommunitySyncConfig,
) -> CommunityInnerSyncReq {
    let metadata = target.metadata.as_ref();
    let focus = extract_source_focus(&report.report_payload);

    CommunityInnerSyncReq {
        name: target.name.clone(),
        website_url: non_empty_option(target.homepage_url.clone()),
        contact_info: metadata_string(metadata, "contact_info")
            .or_else(|| repository_contact_info(target)),
        status: config.status.clone(),
        build_date: metadata_string(metadata, "build_date"),
        function_description: metadata_string(metadata, "function_description")
            .or_else(|| Some(default_function_description(target))),
        necessity_introduction: metadata_string(metadata, "necessity_introduction"),
        introduction_department: metadata_string(metadata, "introduction_department"),
        report_status: non_empty_option(Some(report.status.clone())),
        risk_level: non_empty_option(Some(report.overall_risk.clone())),
        confidence: non_empty_option(Some(report.confidence.clone())),
        summary: non_empty_option(Some(report.summary.clone())),
        organization_structure: focus.organization_structure,
        foundation_info: focus.foundation_info,
        operator_info: metadata_string(metadata, "operator_info").or(focus.operator_info),
        version_lifecycle: focus.version_lifecycle,
        license_info: focus.license_info,
        cla_info: focus.cla_info,
        inner_secret: config.inner_secret.clone(),
    }
}

pub fn parse_community_inner_sync_response(body: &str) -> Result<SbomCommunitySyncResponse> {
    let response = serde_json::from_str::<SbomCommunitySyncResponse>(body)
        .context("parse SBOM community sync response failed")?;

    if response.code != 0 {
        return Err(anyhow!(
            "SBOM community sync failed: code={}, msg={}",
            response.code,
            response.msg
        ));
    }

    Ok(response)
}

#[derive(Debug, Default, PartialEq, Eq)]
struct SourceFocus {
    organization_structure: Option<String>,
    foundation_info: Option<String>,
    operator_info: Option<String>,
    version_lifecycle: Option<String>,
    license_info: Option<String>,
    cla_info: Option<String>,
}

fn extract_source_focus(report_payload: &Value) -> SourceFocus {
    let mut focus = SourceFocus::default();
    let Some(raw_evidence) = report_payload.get("raw_evidence").and_then(Value::as_array) else {
        return focus;
    };

    for entry in raw_evidence {
        let Some(data) = entry.get("data") else {
            continue;
        };

        fill_once(
            &mut focus.organization_structure,
            data_string(data, "organization_structure"),
        );
        fill_once(
            &mut focus.foundation_info,
            data_string(data, "foundation_status").or_else(|| data_string(data, "foundation_info")),
        );
        fill_once(
            &mut focus.operator_info,
            data_string(data, "operator_info")
                .or_else(|| data_string(data, "operator_supply_risk")),
        );
        fill_once(
            &mut focus.version_lifecycle,
            data_string(data, "version_lifecycle"),
        );
        fill_once(
            &mut focus.license_info,
            data_string(data, "license_policy").or_else(|| data_string(data, "license_info")),
        );
        fill_once(
            &mut focus.cla_info,
            data_string(data, "cla_policy").or_else(|| data_string(data, "cla_info")),
        );
    }

    focus
}

fn repository_contact_info(target: &ecosystem_targets::Model) -> Option<String> {
    match (&target.platform, &target.owner, &target.repo) {
        (Some(platform), Some(owner), Some(repo)) if platform.eq_ignore_ascii_case("gitee") => {
            Some(format!("https://gitee.com/{owner}/{repo}"))
        }
        (Some(platform), Some(owner), Some(repo)) if platform.eq_ignore_ascii_case("github") => {
            Some(format!("https://github.com/{owner}/{repo}"))
        }
        (_, Some(owner), Some(repo)) => Some(format!("{owner}/{repo}")),
        _ => None,
    }
}

fn default_function_description(target: &ecosystem_targets::Model) -> String {
    match target.target_type.as_str() {
        "community" => format!("{} 开源社区生态评估目标", target.name),
        "platform" => format!("{} 开源平台生态评估目标", target.name),
        _ => format!("{} 生态评估目标", target.name),
    }
}

fn metadata_string(metadata: Option<&Value>, key: &str) -> Option<String> {
    metadata.and_then(|value| data_string(value, key))
}

fn data_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn non_empty_option(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn fill_once(slot: &mut Option<String>, value: Option<String>) {
    if slot.is_none() {
        *slot = value;
    }
}

fn env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn required_env(key: &str) -> Result<String> {
    optional_env(key)
        .ok_or_else(|| anyhow!("{key} is required when SBOM community sync is enabled"))
}

fn optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use serial_test::serial;
    use std::ffi::OsString;

    struct EnvGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }

        fn remove(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn target(metadata: Option<Value>) -> ecosystem_targets::Model {
        let now = Utc::now();
        ecosystem_targets::Model {
            id: 1,
            name: "openEuler Community".to_string(),
            target_type: "community".to_string(),
            platform: Some("gitee".to_string()),
            role: "governance".to_string(),
            homepage_url: Some("https://www.openeuler.org".to_string()),
            api_base_url: Some("https://gitee.com/api/v5".to_string()),
            owner: Some("openeuler".to_string()),
            repo: Some("community".to_string()),
            default_branch: Some("master".to_string()),
            status: "active".to_string(),
            refresh_interval_hours: 24,
            rule_profile: "openeuler_community".to_string(),
            metadata,
            last_collected_at: None,
            last_report_at: None,
            last_error: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn report(payload: Value) -> ecosystem_reports::Model {
        let now = Utc::now();
        ecosystem_reports::Model {
            id: 10,
            target_id: 1,
            report_type: "ecosystem_profile".to_string(),
            status: "completed".to_string(),
            overall_risk: "HIGH".to_string(),
            confidence: "LOW".to_string(),
            summary: "生态评估摘要".to_string(),
            dimensions: json!({}),
            evidence_summary: Some(json!({"evidence_count": 5})),
            report_payload: payload,
            generated_at: now,
            created_at: now,
            updated_at: now,
        }
    }

    fn config() -> SbomCommunitySyncConfig {
        SbomCommunitySyncConfig {
            endpoint_url: "http://sbom.internal/airspm/community/inner-sync".to_string(),
            inner_secret: "secret".to_string(),
            timeout: Duration::from_secs(5),
            status: None,
        }
    }

    #[test]
    fn build_request_maps_report_payload_and_metadata() {
        let target = target(Some(json!({
            "build_date": "2020-03-27",
            "contact_info": "https://gitee.com/openeuler",
            "operator_info": "由开放原子开源基金会托管运营。",
            "introduction_department": "基础架构部",
            "necessity_introduction": "基础底座"
        })));
        let report = report(json!({
            "raw_evidence": [
                {
                    "data": {
                        "organization_structure": "委员会 + SIG 分层治理",
                        "foundation_status": "开放原子开源基金会孵化并运营"
                    }
                },
                {
                    "data": {
                        "version_lifecycle": "LTS 版本提供长期支持",
                        "license_policy": "Mulan PSL v2",
                        "cla_policy": "贡献前需要签署 CLA"
                    }
                }
            ]
        }));

        let req = build_community_inner_sync_request(&target, &report, &config());

        assert_eq!(req.name, "openEuler Community");
        assert_eq!(
            req.website_url.as_deref(),
            Some("https://www.openeuler.org")
        );
        assert_eq!(
            req.contact_info.as_deref(),
            Some("https://gitee.com/openeuler")
        );
        assert_eq!(req.build_date.as_deref(), Some("2020-03-27"));
        assert_eq!(req.report_status.as_deref(), Some("completed"));
        assert_eq!(req.risk_level.as_deref(), Some("HIGH"));
        assert_eq!(req.confidence.as_deref(), Some("LOW"));
        assert_eq!(
            req.organization_structure.as_deref(),
            Some("委员会 + SIG 分层治理")
        );
        assert_eq!(
            req.foundation_info.as_deref(),
            Some("开放原子开源基金会孵化并运营")
        );
        assert_eq!(
            req.operator_info.as_deref(),
            Some("由开放原子开源基金会托管运营。")
        );
        assert_eq!(
            req.version_lifecycle.as_deref(),
            Some("LTS 版本提供长期支持")
        );
        assert_eq!(req.license_info.as_deref(), Some("Mulan PSL v2"));
        assert_eq!(req.cla_info.as_deref(), Some("贡献前需要签署 CLA"));
        assert_eq!(req.inner_secret, "secret");
        assert_eq!(req.status, None);
    }

    #[test]
    fn build_request_omits_empty_status_and_uses_repository_contact_fallback() {
        let mut config = config();
        config.status = Some("可信".to_string());
        let target = target(None);
        let report = report(json!({}));

        let req = build_community_inner_sync_request(&target, &report, &config);

        assert_eq!(req.status.as_deref(), Some("可信"));
        assert_eq!(
            req.contact_info.as_deref(),
            Some("https://gitee.com/openeuler/community")
        );
        assert_eq!(
            req.function_description.as_deref(),
            Some("openEuler Community 开源社区生态评估目标")
        );
    }

    #[test]
    fn response_parser_rejects_business_failure_even_when_http_is_success() {
        let err = parse_community_inner_sync_response(r#"{"code":1,"msg":"无权访问"}"#)
            .expect_err("business failure must be an error");

        assert!(err.to_string().contains("无权访问"));
    }

    #[test]
    fn response_parser_accepts_success_payload() {
        let response = parse_community_inner_sync_response(
            r#"{"code":0,"msg":"同步社区信息成功","data":{"community_id":"1","name":"openEuler Community","action":"updated"}}"#,
        )
        .unwrap();

        assert_eq!(response.code, 0);
        assert_eq!(
            response.data.and_then(|data| data.action),
            Some("updated".to_string())
