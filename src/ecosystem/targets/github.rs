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
            json!({
                "source_type": "github_ip_policy",
                "source_name": "github_ip_policy",
                "source_url": GITHUB_TERMS_URL,
                "assessment_category": "source",
                "assessment_subcategory": "ip_policy",
                "data": {
                    "ip_policy": ip_policy["summary"],
                    "users_own_content": ip_policy["users_own_content"],
                    "github_retains_platform_ip": ip_policy["github_retains_platform_ip"],
                    "license_grant_to_host_content": ip_policy["license_grant_to_host_content"],
                    "ip_keyword_lines": ip_policy["ip_keyword_lines"],
                    "terms_http_status": terms_page.http_status,
                    "terms_error": terms_page.error,
                }
            }),
            json!({
                "source_type": "github_government_takedown",
                "source_name": "github_government_takedown",
                "source_url": GITHUB_GOV_TAKEDOWN_URL,
                "assessment_category": "source",
                "assessment_subcategory": "government_takedown_policy",
                "data": {
                    "government_takedown_policy": government_takedown["summary"],
                    "supports_geographic_limit": government_takedown["supports_geographic_limit"],
                    "supports_user_appeal": government_takedown["supports_user_appeal"],
                    "publishes_public_requests": government_takedown["publishes_public_requests"],
                    "government_keyword_lines": gov_page.keyword_lines,
                    "government_http_status": gov_page.http_status,
                    "government_error": gov_page.error,
                }
            }),
            json!({
                "source_type": "github_license_policy",
                "source_name": "github_license_policy",
                "source_url": GITHUB_LICENSING_URL,
                "assessment_category": "source",
                "assessment_subcategory": "license_policy",
                "data": {
                    "license_policy": license_policy["summary"],
                    "supports_choosealicense": license_policy["supports_choosealicense"],
                    "supports_license_detection": license_policy["supports_license_detection"],
                    "mentions_default_copyright_rule": license_policy["mentions_default_copyright_rule"],
                    "license_keyword_lines": licensing_page.keyword_lines,
                    "licensing_http_status": licensing_page.http_status,
                    "licensing_error": licensing_page.error,
                }
            }),
            json!({
                "source_type": "github_copyright_policy",
                "source_name": "github_copyright_policy",
                "source_url": GITHUB_DMCA_URL,
                "assessment_category": "source",
                "assessment_subcategory": "copyright_policy",
                "data": {
                    "copyright_info": copyright_info["summary"],
                    "dmca_safe_harbor_mentioned": copyright_info["dmca_safe_harbor_mentioned"],
                    "counter_notice_supported": copyright_info["counter_notice_supported"],
                    "github_copyright_notice_mentioned": copyright_info["github_copyright_notice_mentioned"],
                    "copyright_keyword_lines": copyright_info["copyright_keyword_lines"],
                    "dmca_http_status": dmca_page.http_status,
                    "dmca_error": dmca_page.error,
                }
            }),
            json!({
                "source_type": "github_gov_takedown_archive",
                "source_name": "github_gov_takedown_archive",
                "source_url": "https://github.com/github/gov-takedowns",
                "assessment_category": "source",
                "assessment_subcategory": "government_takedown_archive",
                "data": {
                    "total_requests": gov_takedown_stats["total_requests"],
                    "requests_by_requester": gov_takedown_stats["requests_by_requester"],
                    "truncated": gov_takedown_stats["truncated"],
                    "data_source": "github/gov-takedowns",
                    "archive_error": gov_takedown_stats["error"],
                }
            }),
        ]
    }

    async fn fetch_page(&self, client: &Client, url: &str, keywords: &[&str]) -> PageSnapshot {
        match client.get(url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                match response.text().await {
                    Ok(body) => {
                        let plain_text = strip_tags(&body);
                        PageSnapshot {
                            http_status: Some(status),
                            keyword_lines: extract_keyword_lines(&plain_text, keywords, 12),
                            plain_text,
                            error: None,
                        }
                    }
                    Err(error) => PageSnapshot {
                        http_status: Some(status),
                        keyword_lines: Vec::new(),
                        plain_text: String::new(),
                        error: Some(error.to_string()),
                    },
                }
            }
            Err(error) => PageSnapshot {
                http_status: None,
                keyword_lines: Vec::new(),
                plain_text: String::new(),
                error: Some(error.to_string()),
            },
        }
    }

    fn log_page_result(&self, label: &str, page: &PageSnapshot) {
        match &page.error {
            Some(error) => warn!(
                page = label,
                http_status = ?page.http_status,
                error = %error,
                "GitHub 页面抓取失败"
            ),
            None => info!(
                page = label,
                http_status = ?page.http_status,
                keyword_lines = ?page.keyword_lines,
                "GitHub 页面抓取成功"
            ),
        }
        debug!(
            page = label,
            plain_text_preview = %page.plain_text.chars().take(240).collect::<String>(),
            "GitHub 页面文本预览"
        );
    }

    fn detect_basic_info(&self, about_page: &PageSnapshot) -> Value {
        let text = about_page.plain_text.as_str();
        let lower = text.to_ascii_lowercase();
        let has_platform_intro = lower.contains("complete developer platform")
            || lower.contains("build, scale, and deliver secure software")
            || lower.contains("cloud-based platform")
            || lower.contains("store, share, and work together with others to write code");
        let developer_scale = if let Some(value) = extract_metric(text, "Developers") {
            Some(value)
        } else {
