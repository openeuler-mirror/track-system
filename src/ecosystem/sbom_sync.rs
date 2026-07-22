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
