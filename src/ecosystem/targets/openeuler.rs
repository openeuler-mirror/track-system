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
                    "长期支持版本",
                    "创新版本",
                    "发布间隔周期定为4年",
                    "提供4年社区支持",
                    "每隔12个月会发布一个社区创新版本",
                    "提供6个月社区支持",
                    "生命周期6年",
                    "延长至8年",
                    "SP",
                    "released every two years",
                    "community support for four years",
                    "released every six months",
                    "extended support",
                ],
            )
            .await;
        self.log_page_result("openEuler lifecycle", &lifecycle_page);
        let contribution_page = self
            .fetch_page(
                &client,
                OPENEULER_CONTRIBUTION_URL,
                &[
                    "CLA",
                    "Contributor License Agreement",
                    "Sign the CLA",
                    "Individual CLA",
                    "Corporate CLA",
                    "Employee CLA",
                ],
            )
            .await;
        self.log_page_result("openEuler contribution", &contribution_page);
        let docs_terms_page = self
            .fetch_page(
                &client,
                OPENEULER_DOCS_TERMS_URL,
                &[
                    "CC BY-SA 4.0",
                    "Creative Commons",
                    "MulanPSL2",
                    "Trademarks",
                ],
            )
            .await;
        self.log_page_result("openEuler docs terms", &docs_terms_page);
        let license_text = self
            .fetch_raw_text(&client, OPENEULER_GITEE_LICENSE_URL)
            .await;
        self.log_page_result("openEuler license", &license_text);

        Ok(self.build_evidence_records(
            about_page,
            organization_page,
            foundation_page,
            lifecycle_page,
            contribution_page,
            docs_terms_page,
            license_text,
        ))
    }

    fn build_evidence_records(
        &self,
        about_page: PageSnapshot,
        organization_page: PageSnapshot,
        foundation_page: PageSnapshot,
        lifecycle_page: PageSnapshot,
        contribution_page: PageSnapshot,
        docs_terms_page: PageSnapshot,
        license_text: PageSnapshot,
    ) -> Vec<Value> {
        let organization_structure = self.detect_organization_structure(&organization_page);
        let foundation_status = self.detect_foundation_status(&about_page, &foundation_page);
        let version_lifecycle = self.detect_version_lifecycle(&lifecycle_page);
        let license_policy =
            self.detect_license_policy(&license_text, &docs_terms_page, &contribution_page);
        let cla_policy = self.detect_cla_policy(&contribution_page);

        debug!(
            organization_structure = %organization_structure["summary"].as_str().unwrap_or(""),
            foundation_status = %foundation_status["summary"].as_str().unwrap_or(""),
            version_lifecycle = %version_lifecycle["summary"].as_str().unwrap_or(""),
            license_policy = %license_policy["summary"].as_str().unwrap_or(""),
            cla_policy = %cla_policy["summary"].as_str().unwrap_or(""),
            "openEuler 社区来源评估关键信息提取完成"
        );
        debug!(
            lifecycle_http_status = ?lifecycle_page.http_status,
            lifecycle_keyword_lines = ?lifecycle_page.keyword_lines,
            lifecycle_summary = %version_lifecycle["summary"].as_str().unwrap_or(""),
            has_lts_policy = ?version_lifecycle["has_lts_policy"].as_bool(),
            lts_every_four_years = ?version_lifecycle["lts_every_four_years"].as_bool(),
            lts_every_two_years = ?version_lifecycle["lts_every_two_years"].as_bool(),
            lts_support_four_years = ?version_lifecycle["lts_support_four_years"].as_bool(),
            lts_lifecycle_six_years = ?version_lifecycle["lts_lifecycle_six_years"].as_bool(),
            lts_extendable_to_eight_years = ?version_lifecycle["lts_extendable_to_eight_years"].as_bool(),
            innovation_every_twelve_months = ?version_lifecycle["innovation_every_twelve_months"].as_bool(),
            innovation_every_six_months = ?version_lifecycle["innovation_every_six_months"].as_bool(),
            innovation_support_six_months = ?version_lifecycle["innovation_support_six_months"].as_bool(),
            sp_policy_mentioned = ?version_lifecycle["sp_policy_mentioned"].as_bool(),
            "openEuler 生命周期识别详情"
        );

        vec![
            json!({
                "source_type": "openeuler_community_organization",
                "source_name": "openeuler_community",
                "source_url": OPENEULER_ORGANIZATION_URL,
                "assessment_category": "source",
                "assessment_subcategory": "community_organization",
                "data": {
                    "organization_structure": organization_structure["summary"],
                    "detected_committees": organization_structure["detected_committees"],
                    "organization_keyword_lines": organization_page.keyword_lines,
                    "organization_http_status": organization_page.http_status,
                    "organization_error": organization_page.error,
                }
            }),
            json!({
                "source_type": "openeuler_foundation_profile",
                "source_name": "openeuler_foundation",
                "source_url": OPENEULER_FOUNDATION_URL,
                "assessment_category": "source",
                "assessment_subcategory": "foundation",
                "data": {
                    "foundation_status": foundation_status["summary"],
                    "about_mentions_openatom": foundation_status["about_mentions_openatom"],
                    "foundation_page_mentions_openatom": foundation_status["foundation_page_mentions_openatom"],
                    "foundation_consistency": foundation_status["consistency"],
                    "about_keyword_lines": about_page.keyword_lines,
                    "foundation_keyword_lines": foundation_page.keyword_lines,
                    "about_http_status": about_page.http_status,
                    "foundation_http_status": foundation_page.http_status,
                    "foundation_errors": [about_page.error, foundation_page.error],
                }
            }),
            json!({
                "source_type": "openeuler_lifecycle_policy",
                "source_name": "openeuler_lifecycle",
                "source_url": OPENEULER_LIFECYCLE_URL,
                "assessment_category": "source",
                "assessment_subcategory": "lifecycle_policy",
                "data": {
                    "version_lifecycle": version_lifecycle["summary"],
                    "lifecycle_source_preview": version_lifecycle["source_preview"],
                    "has_lts_policy": version_lifecycle["has_lts_policy"],
                    "lts_every_four_years": version_lifecycle["lts_every_four_years"],
                    "lts_every_two_years": version_lifecycle["lts_every_two_years"],
                    "lts_support_four_years": version_lifecycle["lts_support_four_years"],
                    "lts_lifecycle_six_years": version_lifecycle["lts_lifecycle_six_years"],
                    "lts_extendable_to_eight_years": version_lifecycle["lts_extendable_to_eight_years"],
                    "innovation_every_twelve_months": version_lifecycle["innovation_every_twelve_months"],
                    "innovation_every_six_months": version_lifecycle["innovation_every_six_months"],
                    "innovation_support_six_months": version_lifecycle["innovation_support_six_months"],
                    "extended_support_mentioned": version_lifecycle["extended_support_mentioned"],
                    "sp_policy_mentioned": version_lifecycle["sp_policy_mentioned"],
                    "lifecycle_keyword_lines": lifecycle_page.keyword_lines,
                    "lifecycle_http_status": lifecycle_page.http_status,
                    "lifecycle_error": lifecycle_page.error,
                }
            }),
            json!({
                "source_type": "openeuler_license_policy",
                "source_name": "openeuler_license",
                "source_url": OPENEULER_GITEE_LICENSE_URL,
                "assessment_category": "source",
                "assessment_subcategory": "license_policy",
                "data": {
                    "license_policy": license_policy["summary"],
                    "community_repo_license_detected": license_policy["community_repo_license_detected"],
                    "docs_license_detected": license_policy["docs_license_detected"],
                    "site_footer_license_detected": license_policy["site_footer_license_detected"],
                    "license_keyword_lines": license_policy["license_keyword_lines"],
                    "docs_terms_keyword_lines": docs_terms_page.keyword_lines,
                    "license_error": license_text.error,
                    "docs_terms_error": docs_terms_page.error,
                }
            }),
            json!({
                "source_type": "openeuler_cla_policy",
                "source_name": "openeuler_contribution",
                "source_url": OPENEULER_CONTRIBUTION_URL,
                "assessment_category": "source",
                "assessment_subcategory": "cla_policy",
                "data": {
                    "cla_policy": cla_policy["summary"],
                    "cla_required": cla_policy["cla_required"],
                    "cla_types": cla_policy["cla_types"],
                    "cla_keyword_lines": contribution_page.keyword_lines,
                    "contribution_http_status": contribution_page.http_status,
                    "contribution_error": contribution_page.error,
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
                        let plain_text = if url == OPENEULER_LIFECYCLE_URL {
                            self.build_lifecycle_plain_text(client, &body).await
                        } else {
                            strip_tags(&body)
                        };
                        PageSnapshot {
                            http_status: Some(status),
                            keyword_lines: extract_keyword_lines(&plain_text, keywords, 12),
                            plain_text,
                            raw_body: body,
                            error: None,
                        }
                    }
                    Err(error) => PageSnapshot {
                        http_status: Some(status),
                        keyword_lines: Vec::new(),
                        plain_text: String::new(),
                        raw_body: String::new(),
                        error: Some(error.to_string()),
                    },
                }
            }
            Err(error) => PageSnapshot {
                http_status: None,
                keyword_lines: Vec::new(),
                plain_text: String::new(),
                raw_body: String::new(),
                error: Some(error.to_string()),
            },
        }
    }
