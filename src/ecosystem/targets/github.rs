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
            )
            .await;
        self.log_page_result("github government takedown", &gov_page);
        let terms_page = self
            .fetch_page(
                &client,
                GITHUB_TERMS_URL,
                &[
                    "user-generated content",
                    "you own the content you post on github",
                    "copyright & dmca policy",
                    "intellectual property notice",
                    "copyright",
                    "license grant",
                ],
            )
            .await;
        self.log_page_result("github terms", &terms_page);
        let licensing_page = self
            .fetch_page(
                &client,
                GITHUB_LICENSING_URL,
                &[
                    "choosealicense.com",
                    "without a license",
                    "licensee",
                    "licenses api",
                    "spdx",
                    "default copyright laws apply",
                ],
            )
            .await;
        self.log_page_result("github licensing", &licensing_page);
        let dmca_page = self
            .fetch_page(
                &client,
                GITHUB_DMCA_URL,
                &[
                    "dmca",
                    "safe harbor",
                    "counter notice",
                    "public repository",
                    "takedown notice",
                    "copyright infringement",
                ],
            )
            .await;
        self.log_page_result("github dmca", &dmca_page);

        let gov_takedown_stats = self
            .collect_gov_takedown_stats(&client, github_token.as_deref())
            .await;

        Ok(self.build_evidence_records(
            about_page,
            corporate_page,
            trade_page,
            gov_page,
            terms_page,
            licensing_page,
            dmca_page,
            gov_takedown_stats,
        ))
    }

    fn build_evidence_records(
        &self,
        about_page: PageSnapshot,
        corporate_page: PageSnapshot,
        trade_page: PageSnapshot,
        gov_page: PageSnapshot,
        terms_page: PageSnapshot,
        licensing_page: PageSnapshot,
        dmca_page: PageSnapshot,
        gov_takedown_stats: Value,
    ) -> Vec<Value> {
        let basic_info = self.detect_basic_info(&about_page);
        let corporate_profile = self.detect_corporate_profile(&corporate_page);
        let trade_controls = self.detect_trade_controls(&trade_page);
        let ip_policy = self.detect_ip_policy(&terms_page, &dmca_page);
        let government_takedown = self.detect_government_takedown(&gov_page);
        let license_policy = self.detect_license_policy(&licensing_page, &terms_page);
        let copyright_info = self.detect_copyright_info(&terms_page, &dmca_page);

        debug!(
            basic_info = %basic_info["summary"].as_str().unwrap_or(""),
            organization_structure = %corporate_profile["organization_structure"].as_str().unwrap_or(""),
            foundation_status = %corporate_profile["foundation_status"].as_str().unwrap_or(""),
            trade_controls = %trade_controls["summary"].as_str().unwrap_or(""),
            ip_policy = %ip_policy["summary"].as_str().unwrap_or(""),
            government_takedown = %government_takedown["summary"].as_str().unwrap_or(""),
            license_policy = %license_policy["summary"].as_str().unwrap_or(""),
            copyright_info = %copyright_info["summary"].as_str().unwrap_or(""),
            "GitHub 平台生态目标关键信息提取完成"
        );

        vec![
            json!({
                "source_type": "github_platform_overview",
                "source_name": "github_platform_overview",
                "source_url": GITHUB_ABOUT_URL,
                "assessment_category": "source",
                "assessment_subcategory": "platform_overview",
                "data": {
                    "platform_intro": basic_info["summary"],
                    "about_keyword_lines": about_page.keyword_lines,
                    "about_http_status": about_page.http_status,
                    "about_error": about_page.error,
                }
            }),
            json!({
                "source_type": "github_corporate_profile",
                "source_name": "github_corporate_profile",
                "source_url": GITHUB_CORPORATE_URL,
                "assessment_category": "source",
                "assessment_subcategory": "corporate_profile",
                "data": {
                    "organization_structure": corporate_profile["organization_structure"],
                    "foundation_status": corporate_profile["foundation_status"],
                    "microsoft_acquisition_completed": corporate_profile["microsoft_acquisition_completed"],
                    "operates_independently_as_business": corporate_profile["operates_independently_as_business"],
                    "ceo_mentioned": corporate_profile["ceo_mentioned"],
                    "corporate_keyword_lines": corporate_page.keyword_lines,
                    "corporate_http_status": corporate_page.http_status,
                    "corporate_error": corporate_page.error,
                }
            }),
            json!({
                "source_type": "github_trade_controls",
                "source_name": "github_trade_controls",
                "source_url": GITHUB_TRADE_CONTROLS_URL,
                "assessment_category": "source",
                "assessment_subcategory": "trade_controls",
                "data": {
                    "trade_controls": trade_controls["summary"],
                    "ofac_license_for_iran": trade_controls["ofac_license_for_iran"],
                    "public_repo_access_in_sanctioned_regions": trade_controls["public_repo_access_in_sanctioned_regions"],
                    "itar_restriction_mentioned": trade_controls["itar_restriction_mentioned"],
                    "restricted_regions_mentioned": trade_controls["restricted_regions_mentioned"],
                    "trade_keyword_lines": trade_page.keyword_lines,
                    "trade_http_status": trade_page.http_status,
                    "trade_error": trade_page.error,
                }
            }),
