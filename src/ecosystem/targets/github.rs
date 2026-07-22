use anyhow::Result;
use regex::Regex;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{debug, info, warn};

use crate::ecosystem::targets::configured_fetch_timeout;
use crate::entities::ecosystem_targets;

const GITHUB_ABOUT_URL: &str =
    "https://docs.github.com/en/get-started/start-your-journey/about-github-and-git";
const GITHUB_CORPORATE_URL: &str = "https://github.blog/2018-10-26-github-and-microsoft/";
const GITHUB_TRADE_CONTROLS_URL: &str =
    "https://docs.github.com/en/site-policy/other-site-policies/github-and-trade-controls";
const GITHUB_GOV_TAKEDOWN_URL: &str =
    "https://docs.github.com/en/site-policy/other-site-policies/github-government-takedown-policy";
const GITHUB_TERMS_URL: &str =
    "https://docs.github.com/en/site-policy/github-terms/github-terms-of-service";
const GITHUB_LICENSING_URL: &str =
    "https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/licensing-a-repository";
const GITHUB_DMCA_URL: &str =
    "https://docs.github.com/en/site-policy/content-removal-policies/dmca-takedown-policy";
const DEFAULT_TIMEOUT_SECS: u64 = 40;
const GITHUB_GOV_TAKEDOWNS_API: &str =
    "https://api.github.com/repos/github/gov-takedowns/git/trees/HEAD?recursive=1";

#[derive(Debug, Clone)]
struct PageSnapshot {
    http_status: Option<u16>,
    keyword_lines: Vec<String>,
    plain_text: String,
    error: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct GitHubPlatformCollector;

impl GitHubPlatformCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn matches_target(target: &ecosystem_targets::Model) -> bool {
        let name_key = normalize_lookup_key(&target.name);
        let target_type_key = normalize_lookup_key(&target.target_type);
        let rule_profile_key = normalize_lookup_key(&target.rule_profile);
        let platform_key = target
            .platform
            .as_deref()
            .map(normalize_lookup_key)
            .unwrap_or_default();
        let homepage_key = target
            .homepage_url
            .as_deref()
            .map(normalize_lookup_key)
            .unwrap_or_default();

        let explicit_name_match = matches!(
            name_key.as_str(),
            "github" | "githubplatform" | "githubcommunity"
        );
        let explicit_profile_match =
            rule_profile_key == "githubplatform" || rule_profile_key.contains("githubplatform");
        let explicit_homepage_match = homepage_key.contains("githubcomabout")
            || homepage_key.contains("docsgithubcomensitepolicy");
        let is_platform_target = target_type_key == "platform";

        (explicit_name_match || explicit_profile_match || explicit_homepage_match)
            && (platform_key == "github" || explicit_profile_match || is_platform_target)
            && is_platform_target
    }

    pub async fn collect(&self, target: &ecosystem_targets::Model) -> Result<Vec<Value>> {
        if !Self::matches_target(target) {
            return Ok(Vec::new());
        }

        info!("开始采集 GitHub 平台生态目标信息");
        let github_token = std::env::var("GITHUB_TOKEN")
            .or_else(|_| std::env::var("GITHUB_ACCESS_TOKEN"))
            .ok();
        let client = Client::builder()
            .timeout(configured_fetch_timeout(
                "ECOSYSTEM_GITHUB_FETCH_TIMEOUT_SECS",
                DEFAULT_TIMEOUT_SECS,
            ))
            .user_agent("track-system/ecosystem-github")
            .build()?;

        let about_page = self
            .fetch_page(
                &client,
                GITHUB_ABOUT_URL,
                &[
                    "about github",
                    "cloud-based platform",
                    "store, share, and work together",
                    "repository",
                    "pull requests",
                ],
            )
            .await;
        self.log_page_result("github about", &about_page);
        let corporate_page = self
            .fetch_page(
                &client,
                GITHUB_CORPORATE_URL,
                &[
                    "microsoft acquisition of github is complete",
                    "github will operate independently",
                    "community, platform, and business",
                    "first day as ceo",
                    "future of github",
                ],
            )
            .await;
        self.log_page_result("github corporate profile", &corporate_page);
        let trade_page = self
            .fetch_page(
                &client,
                GITHUB_TRADE_CONTROLS_URL,
                &[
                    "export administration regulations",
                    "OFAC",
                    "sanctioned regions",
                    "public repository services",
                    "ITAR",
                    "trade control",
                ],
            )
            .await;
        self.log_page_result("github trade controls", &trade_page);
        let gov_page = self
            .fetch_page(
                &client,
                GITHUB_GOV_TAKEDOWN_URL,
                &[
                    "government",
                    "takedown",
                    "illegal content",
                    "appeal",
                    "public gov-takedowns repository",
                    "geographic scope",
                ],
