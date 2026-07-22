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
