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

    async fn build_lifecycle_plain_text(&self, client: &Client, html: &str) -> String {
        let direct_text = strip_tags(html);
        if contains_lifecycle_signals(&direct_text) {
            return direct_text;
        }

        let jsonld_text = extract_lifecycle_text_from_raw_body(html);
        if contains_lifecycle_signals(&jsonld_text) {
            return jsonld_text;
        }

        if let Some(asset_path) = extract_vitepress_lifecycle_asset_path(html) {
            let asset_url = to_absolute_asset_url(OPENEULER_LIFECYCLE_URL, &asset_path);
            match client.get(asset_url).send().await {
                Ok(response) => match response.text().await {
                    Ok(body) => {
                        let asset_text = extract_lifecycle_text_from_vitepress_asset(&body);
                        if contains_lifecycle_signals(&asset_text) {
                            return asset_text;
                        }
                    }
                    Err(error) => warn!(error = %error, "读取 openEuler 生命周期资源失败"),
                },
                Err(error) => warn!(error = %error, "抓取 openEuler 生命周期资源失败"),
            }
        }

        if let Some(component_path) = extract_vitepress_lifecycle_component_path(html) {
            let component_url = to_absolute_asset_url(OPENEULER_LIFECYCLE_URL, &component_path);
            match client.get(component_url).send().await {
                Ok(response) => match response.text().await {
                    Ok(body) => {
                        let component_text = extract_lifecycle_text_from_vitepress_component(&body);
                        if contains_lifecycle_signals(&component_text) {
                            return component_text;
                        }
                    }
                    Err(error) => warn!(error = %error, "读取 openEuler 生命周期组件失败"),
                },
                Err(error) => warn!(error = %error, "抓取 openEuler 生命周期组件失败"),
            }
        }

        direct_text
    }

    async fn fetch_raw_text(&self, client: &Client, url: &str) -> PageSnapshot {
        match client.get(url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                match response.text().await {
                    Ok(body) => PageSnapshot {
                        http_status: Some(status),
                        keyword_lines: extract_keyword_lines(
                            &body,
                            &["Mulan PSL v2", "MulanPSL2", "license", "trademark"],
                            8,
                        ),
                        plain_text: body.clone(),
                        raw_body: body,
                        error: None,
                    },
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

    fn log_page_result(&self, label: &str, page: &PageSnapshot) {
        match &page.error {
            Some(error) => warn!(
                page = label,
                http_status = ?page.http_status,
                error = %error,
                "openEuler 页面抓取失败"
            ),
            None => info!(
                page = label,
                http_status = ?page.http_status,
                keyword_lines = ?page.keyword_lines,
                "openEuler 页面抓取成功"
            ),
        }

        debug!(
            page = label,
            plain_text_preview = %page.plain_text.chars().take(240).collect::<String>(),
            "openEuler 页面文本预览"
        );
    }

    fn detect_organization_structure(&self, organization_page: &PageSnapshot) -> Value {
        let lower = organization_page.plain_text.to_ascii_lowercase();
        let candidates = [
            ("openEuler Committee", "openeuler committee"),
            ("Technical Committee", "technical committee"),
            ("Marketing Committee", "marketing committee"),
            ("User Committee", "user committee"),
            ("Security Committee", "security committee"),
            ("SIG", "special interest groups"),
        ];
        let detected_committees = candidates
            .iter()
            .filter_map(|(label, pattern)| {
                if lower.contains(pattern)
                    || (pattern == &"special interest groups" && lower.contains("sig"))
                {
                    Some((*label).to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let summary = if detected_committees.is_empty() {
            "暂未从 openEuler 公开页面识别出清晰的委员会或 SIG 分层结构".to_string()
        } else {
            format!(
                "openEuler 采用委员会 + SIG 的分层治理结构，已识别 {}",
                detected_committees.join("、")
            )
        };

        json!({
            "summary": summary,
            "detected_committees": detected_committees,
        })
    }

    fn detect_foundation_status(
        &self,
        about_page: &PageSnapshot,
        foundation_page: &PageSnapshot,
    ) -> Value {
        let about_text = about_page.plain_text.to_ascii_lowercase();
        let foundation_text = foundation_page.plain_text.to_ascii_lowercase();
        let about_mentions_openatom = about_text.contains("openatom foundation")
            || about_page.plain_text.contains("开放原子开源基金会");
        let foundation_page_mentions_openatom = foundation_text.contains("openatom foundation")
            || foundation_page.plain_text.contains("开放原子开源基金会");
        let about_mentions_incubated =
            about_text.contains("incubated and operated") || about_page.plain_text.contains("孵化");
        let foundation_mentions_graduated = foundation_page.plain_text.contains("毕业项目");
        let consistency = if about_mentions_incubated && foundation_mentions_graduated {
            "POSSIBLY_INCONSISTENT"
        } else if about_mentions_openatom || foundation_page_mentions_openatom {
            "CONSISTENT"
        } else {
            "UNCLEAR"
        };
        let summary = if about_mentions_openatom || foundation_page_mentions_openatom {
            "openEuler 公开页面显示其由开放原子开源基金会孵化并运营，基金会归属关系较明确"
                .to_string()
        } else {
            "暂未从公开页面稳定识别到 openEuler 与开放原子开源基金会的明确归属关系".to_string()
        };

        json!({
            "summary": summary,
            "about_mentions_openatom": about_mentions_openatom,
            "foundation_page_mentions_openatom": foundation_page_mentions_openatom,
            "consistency": consistency,
        })
    }

    fn detect_version_lifecycle(&self, lifecycle_page: &PageSnapshot) -> Value {
        let fallback_text = extract_lifecycle_text_from_raw_body(&lifecycle_page.raw_body);
        let text = if fallback_text.is_empty() {
            lifecycle_page.plain_text.as_str()
        } else {
            fallback_text.as_str()
        };
        let lower = text.to_ascii_lowercase();
        let compact_text = text.replace("**", "").replace([' ', '\n', '\r', '\t'], "");
        let has_lts_policy = lower.contains("lts version")
            || text.contains("长期支持版本")
            || text.contains("LTS版本");
        let lts_every_four_years =
            text.contains("发布间隔周期定为4年") || text.contains("偶数年3月发布新一代LTS首版本");
        let lts_every_two_years = lower.contains("released every two years");
        let lts_support_four_years = lower.contains("community support for four years")
            || lower.contains("lifecycle of a full lts version is four years")
            || text.contains("提供4年社区支持");
        let lts_lifecycle_six_years = compact_text.contains("LTS版本全版本生命周期6年")
            || compact_text.contains("全版本生命周期6年");
        let lts_extendable_to_eight_years =
            text.contains("延长至8年") || text.contains("申请延长至8年");
        let innovation_every_twelve_months =
            text.contains("每隔12个月会发布一个社区创新版本") || text.contains("9月发布创新版本");
        let innovation_every_six_months = lower.contains("released every six months");
        let innovation_support_six_months = lower.contains("community support for six months")
            || text.contains("提供6个月社区支持");
        let extended_support_mentioned = lower.contains("extended support")
            || text.contains("扩展支持")
            || text.contains("维护支持");
        let sp_policy_mentioned = text.contains("SP版本生命周期")
            || text.contains("SP0")
            || text.contains("SP7")
            || text.contains("小 SP")
            || text.contains("大 SP");

        let mut parts = Vec::new();
        if has_lts_policy {
            parts.push("社区版本区分 LTS 版本和创新版本");
        }
        if lts_every_four_years && lts_support_four_years {
            parts.push("自 2025 年 8 月起，LTS 版本发布间隔调整为 4 年，并提供 4 年社区支持");
        } else if lts_every_two_years && lts_support_four_years {
            parts.push("公开页面提到过往 LTS 版本约每两年发布一次并提供四年社区支持");
        }
        if lts_lifecycle_six_years {
            parts.push("LTS 全版本生命周期为 6 年（4+2）");
        }
        if lts_extendable_to_eight_years {
            parts.push("生命周期结束前可申请延长至 8 年");
        }
        if innovation_every_twelve_months && innovation_support_six_months {
            parts.push("自 2025 年 8 月起，创新版本每 12 个月发布一次，并提供 6 个月社区支持");
        } else if innovation_every_six_months && innovation_support_six_months {
            parts.push("公开页面提到过往创新版本约每六个月发布一次并提供六个月支持");
        }
        if extended_support_mentioned {
            parts.push("公开页面提及扩展支持");
        }
        if sp_policy_mentioned {
            parts.push("SP 版本生命周期按大小 SP 区分维护周期");
        }

        json!({
            "summary": if parts.is_empty() {
                "暂未从公开页面识别出明确的版本生命周期策略".to_string()
            } else {
                parts.join("；")
            },
            "source_preview": text.chars().take(240).collect::<String>(),
            "has_lts_policy": has_lts_policy,
            "lts_every_four_years": lts_every_four_years,
            "lts_every_two_years": lts_every_two_years,
            "lts_support_four_years": lts_support_four_years,
            "lts_lifecycle_six_years": lts_lifecycle_six_years,
            "lts_extendable_to_eight_years": lts_extendable_to_eight_years,
            "innovation_every_twelve_months": innovation_every_twelve_months,
            "innovation_every_six_months": innovation_every_six_months,
            "innovation_support_six_months": innovation_support_six_months,
            "extended_support_mentioned": extended_support_mentioned,
            "sp_policy_mentioned": sp_policy_mentioned,
        })
    }

    fn detect_license_policy(
        &self,
        license_text: &PageSnapshot,
        docs_terms_page: &PageSnapshot,
        contribution_page: &PageSnapshot,
    ) -> Value {
        let license_lower = license_text.plain_text.to_ascii_lowercase();
        let docs_lower = docs_terms_page.plain_text.to_ascii_lowercase();
        let contribution_lower = contribution_page.plain_text.to_ascii_lowercase();

        let community_repo_license_detected = if license_lower.contains("mulan psl v2") {
            Some("Mulan PSL v2")
        } else {
            None
        };
        let docs_license_detected = if docs_lower.contains("cc by-sa 4.0") {
            Some("CC BY-SA 4.0")
        } else {
            None
        };
        let site_footer_license_detected = if docs_lower.contains("mulanpsl2") {
            Some("MulanPSL2")
        } else {
            None
        };
        let summary = match (
            community_repo_license_detected,
            docs_license_detected,
            contribution_lower.contains("contributor license agreement"),
        ) {
            (Some(repo_license), Some(docs_license), true) => format!(
                "社区治理仓许可证识别为 {}，文档页面存在 {} 口径，且贡献流程要求签署 CLA",
                repo_license, docs_license
            ),
            (Some(repo_license), _, _) => format!("社区治理仓许可证识别为 {}", repo_license),
            _ => "暂未从公开页面稳定识别出社区治理仓许可证口径".to_string(),
        };

        json!({
            "summary": summary,
            "community_repo_license_detected": community_repo_license_detected,
            "docs_license_detected": docs_license_detected,
            "site_footer_license_detected": site_footer_license_detected,
            "license_keyword_lines": license_text.keyword_lines,
        })
    }

    fn detect_cla_policy(&self, contribution_page: &PageSnapshot) -> Value {
        let lower = contribution_page.plain_text.to_ascii_lowercase();
        let cla_required =
            lower.contains("contributor license agreement") || lower.contains("sign the cla");
        let mut cla_types = Vec::new();
        if lower.contains("individual cla") {
            cla_types.push("Individual CLA".to_string());
        }
        if lower.contains("corporate cla") || lower.contains("corporation cla") {
            cla_types.push("Corporate CLA".to_string());
        }
        if lower.contains("employee cla") {
            cla_types.push("Employee CLA".to_string());
        }

        let summary = if cla_required {
            if cla_types.is_empty() {
                "贡献前需要签署 CLA".to_string()
            } else {
                format!(
                    "贡献前需要签署 CLA，公开页面给出了 {} 等签署类型",
                    cla_types.join("、")
                )
            }
        } else {
            "暂未从公开页面识别出明确的 CLA 签署要求".to_string()
        };

        json!({
            "summary": summary,
            "cla_required": cla_required,
            "cla_types": cla_types,
        })
    }
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
                    "OpenAtom",
                    "Technical Committee",
                    "Security Committee",
                    "LTS",
                    "CLA",
                    "CC BY-SA 4.0",
                    "MulanPSL2",
                ],
                12,
            ),
            plain_text: text.to_string(),
            raw_body: String::new(),
            error: None,
        }
    }

    #[test]
    fn strip_tags_keeps_plain_text() {
        let html =
            "<html><body><h1>openEuler Committee</h1><p>OpenAtom Foundation</p></body></html>";
        let text = strip_tags(html);
        assert!(text.contains("openEuler Committee"));
        assert!(text.contains("OpenAtom Foundation"));
    }

    #[test]
    fn detect_openeuler_lifecycle_from_text() {
        let collector = OpenEulerCommunityCollector::new();
        let page = PageSnapshot {
            http_status: Some(200),
            keyword_lines: Vec::new(),
            plain_text: "社区版本分为长期支持版本和创新版本。长期支持版本自25年8月起生效，发布间隔周期定为4年，提供4年社区支持。LTS版本全版本生命周期6年(4+2)，可申请延长至8年。openEuler每隔12个月会发布一个社区创新版本，提供6个月社区支持。SP版本生命周期原则上按照小SP 9个月、大SP 24个月执行。".to_string(),
            raw_body: String::new(),
            error: None,
        };

        let lifecycle = collector.detect_version_lifecycle(&page);
        assert_eq!(lifecycle["has_lts_policy"], true);
        assert_eq!(lifecycle["lts_every_four_years"], true);
        assert_eq!(lifecycle["lts_lifecycle_six_years"], true);
        assert_eq!(lifecycle["innovation_every_twelve_months"], true);
        assert_eq!(lifecycle["sp_policy_mentioned"], true);
    }

    #[test]
    fn detect_openeuler_cla_types() {
        let collector = OpenEulerCommunityCollector::new();
        let page = PageSnapshot {
            http_status: Some(200),
            keyword_lines: Vec::new(),
            plain_text: "Sign the openEuler Contributor License Agreement (CLA). Individual CLA. Corporate CLA. Employee CLA.".to_string(),
            raw_body: String::new(),
            error: None,
        };

        let cla = collector.detect_cla_policy(&page);
        assert_eq!(cla["cla_required"], true);
        assert_eq!(cla["cla_types"].as_array().map(|v| v.len()), Some(3));
    }

    #[test]
    fn build_evidence_records_maps_openeuler_source_signals() {
        let collector = OpenEulerCommunityCollector::new();
        let about_page = page("openEuler is incubated and operated by the OpenAtom Foundation.");
        let organization_page = page(
            "openEuler Committee, Technical Committee, Marketing Committee, User Committee, Security Committee and Special Interest Groups are part of governance.",
        );
        let foundation_page = page("开放原子开源基金会 openEuler 毕业项目。");
        let lifecycle_page = page(
            "社区版本分为长期支持版本和创新版本。长期支持版本发布间隔周期定为4年，提供4年社区支持。LTS版本全版本生命周期6年(4+2)，可申请延长至8年。openEuler每隔12个月会发布一个社区创新版本，提供6个月社区支持。SP版本生命周期原则上按照小 SP 和大 SP 执行。",
        );
        let contribution_page = page(
            "Sign the openEuler Contributor License Agreement (CLA). Individual CLA. Corporate CLA. Employee CLA.",
        );
        let docs_terms_page = page("Docs are under CC BY-SA 4.0 and footer mentions MulanPSL2.");
        let license_text = page("Mulan PSL v2 license text for openEuler community repository.");

        let evidence = collector.build_evidence_records(
            about_page,
            organization_page,
            foundation_page,
            lifecycle_page,
            contribution_page,
            docs_terms_page,
            license_text,
        );

        assert_eq!(evidence.len(), 5);
        assert_eq!(
            evidence[0]["source_type"],
            "openeuler_community_organization"
        );
        assert_eq!(
            evidence[0]["data"]["detected_committees"]
                .as_array()
                .map(Vec::len),
            Some(6)
        );
        assert_eq!(
            evidence[1]["data"]["foundation_consistency"],
            "POSSIBLY_INCONSISTENT"
        );
        assert_eq!(evidence[2]["data"]["lts_every_four_years"], true);
        assert_eq!(evidence[2]["data"]["sp_policy_mentioned"], true);
        assert_eq!(
            evidence[3]["data"]["community_repo_license_detected"],
            "Mulan PSL v2"
        );
        assert_eq!(evidence[4]["data"]["cla_required"], true);
    }

    #[test]
    fn detect_openeuler_lifecycle_from_jsonld_body() {
        let collector = OpenEulerCommunityCollector::new();
        let page = PageSnapshot {
            http_status: Some(200),
            keyword_lines: Vec::new(),
            plain_text: "openEuler 生命周期管理".to_string(),
            raw_body: r#"
                <html><head>
                <script type="application/ld+json">
                {
                  "@context":"https://schema.org",
                  "@type":"FAQPage",
                  "mainEntity":[
                    {
                      "@type":"Question",
                      "name":"openEuler社区版本生命周期管理规范（LTS+SP）",
                      "acceptedAnswer":{
                        "@type":"Answer",
                        "text":"长期支持版本自25年8月起生效，发布间隔周期定为4年，提供4年社区支持。LTS版本全版本生命周期6年(4+2)，申请延长至8年。openEuler每隔12个月会发布一个社区创新版本，提供6个月社区支持。SP版本生命周期原则上按照小SP 9个月，大SP 24个月执行。"
                      }
                    }
                  ]
                }
                </script>
                </head><body></body></html>
            "#
            .to_string(),
            error: None,
        };

        let lifecycle = collector.detect_version_lifecycle(&page);
        assert_eq!(lifecycle["lts_every_four_years"], true);
        assert_eq!(lifecycle["lts_lifecycle_six_years"], true);
        assert_eq!(lifecycle["innovation_every_twelve_months"], true);
        assert_eq!(lifecycle["sp_policy_mentioned"], true);
    }

    #[test]
    fn extract_lifecycle_text_from_vitepress_asset_extracts_key_points() {
        let asset = r#"const h=JSON.parse('{"title":"openEuler 生命周期管理","description":"探索openEuler软件的生命周期管理","frontmatter":{"title":"openEuler版本规划及生命周期"},"headers":[],"relativePath":"zh/other/lifecycle/index.md","filePath":"zh/other/lifecycle/index.md","lastUpdated":0,"mainEntity":[{"text":"长期支持版本自25年8月起生效，发布间隔周期定为4年，提供4年社区支持。LTS版本全版本生命周期6年(4+2)。openEuler每隔12个月会发布一个社区创新版本，提供6个月社区支持。SP版本生命周期原则上按照小SP 9个月、大SP 24个月执行。"}]}');"#;
        let text = extract_lifecycle_text_from_vitepress_asset(asset);
        assert!(text.contains("发布间隔周期定为4年"));
        assert!(text.contains("生命周期6年"));
        assert!(text.contains("12个月"));
        assert!(text.contains("SP版本生命周期"));
    }

    #[test]
    fn extract_lifecycle_text_from_vitepress_component_extracts_markdown_blocks() {
        let body = r#"const A=`# 1、openEuler社区版本生命周期管理规范（总体）

社区版本分为长期支持版本和创新版本。

- **长期支持版本（自25年8月起生效）：** 发布间隔周期定为4年，提供4年社区支持。
- **社区创新版本（自25年8月起生效）：** openEuler每隔12个月会发布一个社区创新版本，提供6个月社区支持。
`,I=`# 2、openEuler社区版本生命周期管理规范（LTS+SP）

1. LTS版本**全版本**生命周期6年(4+2)，申请延长至8年。
2. LTS 版本 SP 版本生命周期原则上按照小 SP（6月份 Release，可选）9个月，大 SP（12月份 Release）24个月执行。
`;"#;
        let text = extract_lifecycle_text_from_vitepress_component(body);
        assert!(text.contains("发布间隔周期定为4年"));
        assert!(text.contains("12个月会发布一个社区创新版本"));
        assert!(text.contains("生命周期6年"));
        assert!(text.contains("SP 版本生命周期"));
    }
}

fn extract_lifecycle_text_from_raw_body(raw_body: &str) -> String {
    if raw_body.is_empty() {
        return String::new();
    }

    let jsonld_re =
        Regex::new(r#"(?is)<script[^>]*type=["']application/ld\+json["'][^>]*>(.*?)</script>"#)
            .expect("jsonld regex");
    let mut segments = Vec::new();

    for captures in jsonld_re.captures_iter(raw_body) {
        let Some(script_body) = captures.get(1).map(|m| m.as_str()) else {
            continue;
        };
        let Ok(json_value) = serde_json::from_str::<Value>(script_body) else {
            continue;
        };
        collect_text_segments_from_json(&json_value, &mut segments);
    }

    segments.join(" ")
}

fn extract_vitepress_lifecycle_asset_path(raw_body: &str) -> Option<String> {
    let asset_re = Regex::new(
        r#"(?i)(?:href|src)=["']([^"']*zh_other_lifecycle_index\.md\.[^"']*\.lean\.js)["']"#,
    )
    .expect("vitepress asset regex");
    asset_re
        .captures(raw_body)
        .and_then(|captures| captures.get(1).map(|m| m.as_str().to_string()))
}

fn extract_vitepress_lifecycle_component_path(raw_body: &str) -> Option<String> {
    let component_re = Regex::new(r#"(?i)(?:href|src)=["']([^"']*TheLifecycle\.[^"']*\.js)["']"#)
        .expect("vitepress component regex");
    component_re
        .captures(raw_body)
        .and_then(|captures| captures.get(1).map(|m| m.as_str().to_string()))
}

fn to_absolute_asset_url(base_url: &str, asset_path: &str) -> String {
    if asset_path.starts_with("http://") || asset_path.starts_with("https://") {
        return asset_path.to_string();
    }

    let base = reqwest::Url::parse(base_url).expect("valid base url");
    base.join(asset_path).expect("valid asset path").to_string()
}

fn extract_lifecycle_text_from_vitepress_asset(body: &str) -> String {
    let mut segments = Vec::new();

    let json_parse_re =
        Regex::new(r#"JSON\.parse\('((?:\\.|[^'])*)'\)"#).expect("json parse regex");
    for captures in json_parse_re.captures_iter(body) {
        let Some(raw) = captures.get(1).map(|m| m.as_str()) else {
            continue;
        };
        if let Ok(json_value) = serde_json::from_str::<Value>(raw) {
            collect_text_segments_from_json(&json_value, &mut segments);
            continue;
        }
        let Ok(decoded) = serde_json::from_str::<String>(&format!("\"{}\"", raw)) else {
            continue;
        };
        let Ok(json_value) = serde_json::from_str::<Value>(&decoded) else {
            continue;
        };
        collect_text_segments_from_json(&json_value, &mut segments);
    }

    segments.join(" ")
}

fn extract_lifecycle_text_from_vitepress_component(body: &str) -> String {
    let generic_re = Regex::new(r#"(?s)`([^`]*)`"#).expect("generic lifecycle regex");
    let mut segments = Vec::new();
    for captures in generic_re.captures_iter(body) {
        if let Some(matched) = captures.get(1) {
            let text = matched.as_str().replace("\\n", "\n");
            if contains_lifecycle_signals(&text) {
                segments.push(text);
