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
            None
        };
        let org_scale = extract_metric(text, "Organizations");
        let repo_scale = extract_metric(text, "Repositories");
        let summary = if has_platform_intro {
            format!(
                "GitHub 是面向开发者的代码托管、协作开发与软件交付平台{}{}{}",
                developer_scale
                    .as_ref()
                    .map(|v| format!("，公开页面提及 {} Developers", v))
                    .unwrap_or_default(),
                org_scale
                    .as_ref()
                    .map(|v| format!("、{} Organizations", v))
                    .unwrap_or_default(),
                repo_scale
                    .as_ref()
                    .map(|v| format!("、{} Repositories", v))
                    .unwrap_or_default(),
            )
        } else {
            "GitHub 是提供代码托管、协作开发与软件交付能力的全球开发平台".to_string()
        };
        json!({
            "summary": summary,
            "developer_scale": developer_scale,
            "organization_scale": org_scale,
            "repository_scale": repo_scale,
        })
    }

    fn detect_trade_controls(&self, trade_page: &PageSnapshot) -> Value {
        let text = trade_page.plain_text.as_str();
        let lower = text.to_ascii_lowercase();
        let ofac_license_for_iran = lower.contains("license from ofac")
            && (lower.contains("iran") || text.contains("伊朗"));
        let public_repo_access_in_sanctioned_regions = lower.contains("public repository services")
            || lower.contains("free public repository services");
        let itar_restriction_mentioned = lower.contains("itar");
        let restricted_regions_mentioned = lower.contains("crimea")
            || lower.contains("north korea")
            || lower.contains("cuba")
            || lower.contains("russia")
            || lower.contains("belarus");
        let mut parts = vec!["GitHub 平台受美国出口管制与制裁合规约束".to_string()];
        if ofac_license_for_iran {
            parts.push("公开说明提及已获得 OFAC 许可为伊朗开发者恢复云服务".to_string());
        }
        if public_repo_access_in_sanctioned_regions {
            parts.push("在部分受制裁地区仍努力维持公共仓库和开源协作访问".to_string());
        }
        if itar_restriction_mentioned {
            parts.push("GitHub.com 不适合托管 ITAR 受控数据".to_string());
        }
        if restricted_regions_mentioned {
            parts.push("对受制裁国家或地区以及被拒绝方存在访问限制".to_string());
        }
        json!({
            "summary": parts.join("；"),
            "ofac_license_for_iran": ofac_license_for_iran,
            "public_repo_access_in_sanctioned_regions": public_repo_access_in_sanctioned_regions,
            "itar_restriction_mentioned": itar_restriction_mentioned,
            "restricted_regions_mentioned": restricted_regions_mentioned,
        })
    }

    fn detect_corporate_profile(&self, corporate_page: &PageSnapshot) -> Value {
        let text = corporate_page.plain_text.as_str();
        let lower = text.to_ascii_lowercase();
        let microsoft_acquisition_completed = lower
            .contains("microsoft acquisition of github is complete")
            || lower.contains("joining forces with microsoft");
        let operates_independently_as_business = lower
            .contains("github will operate independently")
            && lower.contains("community, platform, and business");
        let ceo_mentioned = lower.contains("first day as ceo") || lower.contains("role as ceo");

        let organization_structure = if microsoft_acquisition_completed
            && operates_independently_as_business
        {
            "GitHub 采用商业公司治理结构，2018 年微软完成收购后作为其子公司继续独立运营，由 CEO 与管理团队负责公司经营和平台发展".to_string()
        } else if microsoft_acquisition_completed {
            "GitHub 在微软收购后按公司化方式运营，由管理层负责平台经营与业务发展".to_string()
        } else {
            "GitHub 以企业化平台运营为主，治理结构由公司管理层和业务团队驱动".to_string()
        };

        let foundation_status = if operates_independently_as_business {
            "未见基金会治理安排；GitHub 官方表述其以 community、platform、business 形态独立运营，属于商业化平台而非基金会项目".to_string()
        } else {
            "未见基金会归属信息，整体更接近商业公司运营模式".to_string()
        };

        json!({
            "organization_structure": organization_structure,
            "foundation_status": foundation_status,
            "microsoft_acquisition_completed": microsoft_acquisition_completed,
            "operates_independently_as_business": operates_independently_as_business,
            "ceo_mentioned": ceo_mentioned,
        })
    }

    fn detect_ip_policy(&self, terms_page: &PageSnapshot, dmca_page: &PageSnapshot) -> Value {
        let terms = terms_page.plain_text.as_str();
        let terms_lower = terms.to_ascii_lowercase();
        let users_own_content = terms_lower.contains("you own the content you post on github")
            || terms_lower.contains("you retain ownership");
        let github_retains_platform_ip = terms_lower.contains("github and our licensors")
            && terms_lower.contains("retain ownership");
        let license_grant_to_host_content =
            terms_lower.contains("license grant to us") || terms_lower.contains("grant us");
        let summary = format!(
            "GitHub 条款明确{}{}{}",
            if users_own_content {
                "用户对其发布内容保有所有权"
            } else {
                "用户内容所有权边界需要结合条款进一步确认"
            },
            if license_grant_to_host_content {
                "，同时需授予 GitHub 托管、展示与解析内容的必要许可"
            } else {
                ""
            },
            if github_retains_platform_ip
                || dmca_page
                    .plain_text
                    .contains("Intellectual Property Notice")
            {
                "；平台自身网站与服务相关知识产权由 GitHub 及其许可方保留"
            } else {
                ""
            }
        );
        json!({
            "summary": summary,
            "users_own_content": users_own_content,
            "github_retains_platform_ip": github_retains_platform_ip,
            "license_grant_to_host_content": license_grant_to_host_content,
            "ip_keyword_lines": extract_keyword_lines(
                terms,
                &["You own the content you post on GitHub", "retain ownership", "license grant", "Intellectual Property Notice", "retain ownership of all intellectual property rights"],
                8
            ),
        })
    }

    fn detect_government_takedown(&self, gov_page: &PageSnapshot) -> Value {
        let text = gov_page.plain_text.as_str();
        let lower = text.to_ascii_lowercase();
        let supports_geographic_limit = lower.contains("geographic scope")
            && (lower.contains("limit") || lower.contains("restrict"));
        let supports_user_appeal = lower.contains("affected users to appeal");
        let publishes_public_requests = lower.contains("public gov-takedowns repository")
            || lower.contains("post the official request");
        let summary = format!(
            "GitHub 设有政府下架请求处理流程，{}{}{}",
            if supports_geographic_limit {
                "优先限制地理范围"
            } else {
                "会按当地法要求处理内容"
            },
            if supports_user_appeal {
                "，并允许受影响用户申诉"
            } else {
                ""
            },
            if publishes_public_requests {
                "；同时会将官方请求公开到 gov-takedowns 仓库以提升透明度"
            } else {
                ""
            }
        );
        json!({
            "summary": summary,
            "supports_geographic_limit": supports_geographic_limit,
            "supports_user_appeal": supports_user_appeal,
            "publishes_public_requests": publishes_public_requests,
        })
    }

    fn detect_license_policy(
        &self,
        licensing_page: &PageSnapshot,
        terms_page: &PageSnapshot,
    ) -> Value {
        let text = licensing_page.plain_text.as_str();
        let lower = text.to_ascii_lowercase();
        let supports_choosealicense = lower.contains("choosealicense.com");
        let supports_license_detection =
            lower.contains("licensee") || lower.contains("licenses api");
        let mentions_default_copyright_rule = lower.contains("default copyright laws apply");
        let summary = format!(
            "GitHub 提供开源许可证选择与识别能力{}{}{}",
            if supports_choosealicense {
                "，包括 Choose a License 指引"
            } else {
                ""
            },
            if supports_license_detection {
                "、Licensee/License API 等许可证识别能力"
            } else {
                ""
            },
            if mentions_default_copyright_rule || terms_page.plain_text.contains("fork") {
                "；若仓库未声明许可证，则默认版权法仍然适用"
            } else {
                ""
            }
        );
        json!({
            "summary": summary,
            "supports_choosealicense": supports_choosealicense,
            "supports_license_detection": supports_license_detection,
            "mentions_default_copyright_rule": mentions_default_copyright_rule,
        })
    }

    async fn collect_gov_takedown_stats(&self, client: &Client, token: Option<&str>) -> Value {
        let mut request = client
            .get(GITHUB_GOV_TAKEDOWNS_API)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(t) = token {
            request = request.bearer_auth(t);
        }
        match request.send().await {
            Err(e) => {
                warn!(error = %e, "gov-takedowns API 请求失败");
                json!({ "error": e.to_string(), "total_requests": null })
            }
            Ok(resp) if !resp.status().is_success() => {
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();
                warn!(status, body = %body, "gov-takedowns API 返回非 2xx");
                json!({ "error": format!("HTTP {}: {}", status, body), "total_requests": null })
            }
            Ok(resp) => match resp.json::<Value>().await {
                Err(e) => {
                    warn!(error = %e, "gov-takedowns API JSON 解析失败");
                    json!({ "error": e.to_string(), "total_requests": null })
                }
                Ok(tree_resp) => parse_gov_takedown_tree(&tree_resp),
            },
        }
    }

    fn detect_copyright_info(&self, terms_page: &PageSnapshot, dmca_page: &PageSnapshot) -> Value {
        let terms = terms_page.plain_text.as_str();
        let dmca = dmca_page.plain_text.as_str();
        let dmca_lower = dmca.to_ascii_lowercase();
        let terms_lower = terms.to_ascii_lowercase();
        let dmca_safe_harbor_mentioned = dmca_lower.contains("safe harbor");
        let counter_notice_supported = dmca_lower.contains("counter notice");
        let github_copyright_notice_mentioned = terms_lower.contains("copyright © github")
            || terms_lower.contains("copyright & dmca policy");
        let summary = format!(
            "GitHub 提供版权投诉与 DMCA 处理机制{}{}{}",
            if dmca_safe_harbor_mentioned {
                "，强调平台维持 DMCA safe harbor 合规"
            } else {
                ""
            },
            if counter_notice_supported {
                "，支持 counter notice 反通知流程"
            } else {
                ""
            },
            if github_copyright_notice_mentioned {
                "；同时条款声明 GitHub 网站和服务外观受 GitHub 版权保护"
            } else {
                ""
            }
        );
        json!({
            "summary": summary,
            "dmca_safe_harbor_mentioned": dmca_safe_harbor_mentioned,
            "counter_notice_supported": counter_notice_supported,
            "github_copyright_notice_mentioned": github_copyright_notice_mentioned,
            "copyright_keyword_lines": extract_keyword_lines(
                &format!("{}\n{}", terms, dmca),
                &["DMCA", "safe harbor", "counter notice", "copyright © GitHub", "copyright infringement"],
                10
            ),
        })
    }
}

/// 将 GitHub Tree API 响应解析为政府下架请求统计。
///
/// 仓库路径约定：每条请求是一个 blob 文件，路径首级目录即"请求方"。
/// 根目录文件（README 等）不计入统计。
fn parse_gov_takedown_tree(tree_resp: &Value) -> Value {
    let truncated = tree_resp
        .get("truncated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let tree = match tree_resp.get("tree").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return json!({ "error": "missing tree field", "total_requests": null }),
    };

    let mut requests_by_requester: std::collections::BTreeMap<String, u64> =
        std::collections::BTreeMap::new();
    let mut total_requests: u64 = 0;

    for entry in tree {
        // 只统计 blob（文件），跳过 tree（目录）节点
        if entry.get("type").and_then(|v| v.as_str()) != Some("blob") {
            continue;
        }
        let path = match entry.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => continue,
        };
        // 首级目录作为请求方；根目录文件（无 '/'）不计入
        let slash_pos = match path.find('/') {
            Some(pos) => pos,
            None => continue,
        };
        let requester = &path[..slash_pos];
        *requests_by_requester
            .entry(requester.to_string())
            .or_insert(0) += 1;
        total_requests += 1;
    }

    json!({
        "total_requests": total_requests,
        "requests_by_requester": requests_by_requester,
        "truncated": truncated,
    })
}

fn extract_metric(text: &str, suffix: &str) -> Option<String> {
    let re = Regex::new(&format!(r"(?i)(\d+[A-Z+.]*)\s+{}", regex::escape(suffix))).ok()?;
    re.captures(text)
        .and_then(|captures| captures.get(1).map(|m| m.as_str().to_string()))
}

fn normalize_lookup_key(input: &str) -> String {
    input
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn strip_tags(text: &str) -> String {
    let script_re = Regex::new(r"(?is)<script.*?>.*?</script>").expect("script regex");
    let style_re = Regex::new(r"(?is)<style.*?>.*?</style>").expect("style regex");
    let break_re = Regex::new(r"(?i)<br\s*/?>").expect("br regex");
    let block_end_re =
        Regex::new(r"(?i)</(p|div|li|tr|td|th|h\d|section|article)>").expect("block regex");
    let tag_re = Regex::new(r"<[^>]+>").expect("tag regex");
    let whitespace_re = Regex::new(r"[ \t]+").expect("whitespace regex");
    let newline_re = Regex::new(r"\n\s*\n+").expect("newline regex");

    let text = script_re.replace_all(text, " ");
    let text = style_re.replace_all(&text, " ");
    let text = break_re.replace_all(&text, "\n");
    let text = block_end_re.replace_all(&text, "\n");
    let text = tag_re.replace_all(&text, " ");
    let text = text.replace("&nbsp;", " ");
    let text = text.replace("&amp;", "&");
    let text = text.replace("&quot;", "\"");
    let text = text.replace("&#39;", "'");
    let text = text.replace('\r', "");
    let text = newline_re.replace_all(&text, "\n");
    whitespace_re.replace_all(&text, " ").trim().to_string()
}

fn extract_keyword_lines(text: &str, keywords: &[&str], max_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if keywords
            .iter()
            .any(|keyword| lower.contains(&keyword.to_ascii_lowercase()))
        {
            let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
            if !lines.contains(&normalized) {
                lines.push(normalized);
            }
            if lines.len() >= max_lines {
                break;
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(text: &str) -> PageSnapshot {
        PageSnapshot {
            http_status: Some(200),
            keyword_lines: extract_keyword_lines(
                text,
                &[
                    "GitHub",
                    "Microsoft",
                    "OFAC",
                    "government",
                    "license",
                    "DMCA",
                ],
                12,
            ),
            plain_text: text.to_string(),
            error: None,
        }
    }

    #[test]
    fn matches_target_accepts_github_aliases() {
        let target = ecosystem_targets::Model {
            id: 1,
            name: "GitHub Platform".to_string(),
            target_type: "platform".to_string(),
            platform: Some("github".to_string()),
            role: "hosting".to_string(),
            homepage_url: Some("https://github.com".to_string()),
            api_base_url: Some("https://api.github.com".to_string()),
            owner: None,
            repo: None,
            default_branch: None,
            status: "active".to_string(),
            refresh_interval_hours: 24,
            rule_profile: "github_platform".to_string(),
            metadata: None,
            last_collected_at: None,
            last_report_at: None,
            last_error: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert!(GitHubPlatformCollector::matches_target(&target));
    }

    #[test]
    fn matches_target_rejects_generic_github_repository() {
        let target = ecosystem_targets::Model {
            id: 2,
            name: "track-system".to_string(),
            target_type: "repository".to_string(),
            platform: Some("github".to_string()),
            role: "application".to_string(),
            homepage_url: Some("https://github.com/example/track-system".to_string()),
            api_base_url: Some("https://api.github.com".to_string()),
            owner: Some("example".to_string()),
            repo: Some("track-system".to_string()),
            default_branch: Some("main".to_string()),
            status: "active".to_string(),
            refresh_interval_hours: 24,
            rule_profile: "default".to_string(),
            metadata: None,
            last_collected_at: None,
            last_report_at: None,
            last_error: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert!(!GitHubPlatformCollector::matches_target(&target));
    }

    #[test]
    fn detect_trade_controls_from_text() {
        let collector = GitHubPlatformCollector::new();
        let page = PageSnapshot {
            http_status: Some(200),
            keyword_lines: Vec::new(),
            plain_text: "GitHub.com may be subject to the U.S. Export Administration Regulations and sanctions laws. GitHub secured a license from OFAC for Iran and continues to keep public repository services available in sanctioned regions. GitHub.com is not designed to host data subject to the ITAR.".to_string(),
            error: None,
        };
        let result = collector.detect_trade_controls(&page);
        assert_eq!(result["ofac_license_for_iran"], true);
        assert_eq!(result["public_repo_access_in_sanctioned_regions"], true);
        assert_eq!(result["itar_restriction_mentioned"], true);
    }

    #[test]
    fn detect_corporate_profile_from_text() {
        let collector = GitHubPlatformCollector::new();
        let page = PageSnapshot {
            http_status: Some(200),
            keyword_lines: Vec::new(),
            plain_text: "I’m thrilled to share that the Microsoft acquisition of GitHub is complete. Monday is my first day as CEO. GitHub will operate independently as a community, platform, and business.".to_string(),
            error: None,
        };
        let result = collector.detect_corporate_profile(&page);
        assert_eq!(result["microsoft_acquisition_completed"], true);
        assert_eq!(result["operates_independently_as_business"], true);
        assert_eq!(result["ceo_mentioned"], true);
        assert!(result["organization_structure"]
            .as_str()
            .unwrap_or("")
            .contains("微软完成收购"));
        assert!(result["foundation_status"]
            .as_str()
            .unwrap_or("")
            .contains("未见基金会"));
    }

    #[test]
    fn detect_government_takedown_from_text() {
        let collector = GitHubPlatformCollector::new();
        let page = PageSnapshot {
