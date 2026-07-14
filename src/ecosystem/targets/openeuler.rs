use anyhow::Result;
use regex::Regex;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{debug, info, warn};

use crate::ecosystem::targets::configured_fetch_timeout;
use crate::entities::ecosystem_targets;

const OPENEULER_ABOUT_URL: &str = "https://www.openeuler.org/en/wiki/about/introduce/";
const OPENEULER_ORGANIZATION_URL: &str = "https://www.openeuler.org/en/community/organization/";
const OPENEULER_CONTRIBUTION_URL: &str =
    "https://www.openeuler.org/en/community/contribution/detail.html";
const OPENEULER_LIFECYCLE_URL: &str = "https://www.openeuler.openatom.cn/zh/other/lifecycle/";
const OPENEULER_DOCS_TERMS_URL: &str =
    "https://docs.openeuler.org/en/docs/22.03_LTS_SP4/server/releasenotes/terms_of_use.html";
const OPENEULER_FOUNDATION_URL: &str = "https://www.openatom.org/project/projectmflp9s714SYZ";
const OPENEULER_GITEE_LICENSE_URL: &str =
    "https://gitee.com/openeuler/community/raw/master/LICENSE";
const DEFAULT_TIMEOUT_SECS: u64 = 20;

#[derive(Debug, Clone)]
struct PageSnapshot {
    http_status: Option<u16>,
    keyword_lines: Vec<String>,
    plain_text: String,
    raw_body: String,
    error: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct OpenEulerCommunityCollector;

impl OpenEulerCommunityCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn matches_target(target: &ecosystem_targets::Model) -> bool {
        let mut text = vec![
            target.name.to_ascii_lowercase(),
            target.target_type.to_ascii_lowercase(),
            target.rule_profile.to_ascii_lowercase(),
        ];
        if let Some(platform) = &target.platform {
            text.push(platform.to_ascii_lowercase());
        }
        if let Some(homepage) = &target.homepage_url {
            text.push(homepage.to_ascii_lowercase());
        }
        text.iter()
            .any(|item| item.contains("openeuler") || item.contains("openEuler"))
    }

    pub async fn collect(&self, target: &ecosystem_targets::Model) -> Result<Vec<Value>> {
        if Self::matches_target(target) {
            return self.collect_openeuler_community(target).await;
        }

        Ok(Vec::new())
    }

    async fn collect_openeuler_community(
        &self,
        _target: &ecosystem_targets::Model,
    ) -> Result<Vec<Value>> {
        info!("开始采集 openEuler 社区来源评估信息");
        let client = Client::builder()
            .timeout(configured_fetch_timeout(
                "ECOSYSTEM_OPENEULER_FETCH_TIMEOUT_SECS",
                DEFAULT_TIMEOUT_SECS,
            ))
            .user_agent("track-system/ecosystem-openeuler")
            .build()?;

        let about_page = self
            .fetch_page(
                &client,
                OPENEULER_ABOUT_URL,
                &[
                    "OpenAtom",
                    "incubated",
                    "operated",
                    "Security Committee",
                    "Special Interest Groups",
                    "Governance",
                ],
            )
            .await;
        self.log_page_result("openEuler about", &about_page);
        let organization_page = self
            .fetch_page(
                &client,
                OPENEULER_ORGANIZATION_URL,
                &[
                    "openEuler Committee",
                    "Technical Committee",
                    "Marketing Committee",
                    "User Committee",
                    "Security Committee",
                    "SIG",
                    "OpenAtom Foundation",
                ],
            )
            .await;
        self.log_page_result("openEuler organization", &organization_page);
        let foundation_page = self
            .fetch_page(
                &client,
                OPENEULER_FOUNDATION_URL,
                &["开放原子开源基金会", "openEuler", "开源欧拉", "项目"],
            )
            .await;
        self.log_page_result("openEuler foundation", &foundation_page);
        let lifecycle_page = self
            .fetch_page(
                &client,
                OPENEULER_LIFECYCLE_URL,
                &[
                    "LTS",
