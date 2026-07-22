use anyhow::Result;
use regex::Regex;
use reqwest::Client;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::ecosystem::targets::configured_fetch_timeout;
use crate::entities::ecosystem_targets;

const ATOMGIT_DOCS_HOME_URL: &str = "https://docs.atomgit.com/en/";
const ATOMGIT_TERMS_URL: &str = "https://docs.atomgit.com/docs/help/home/protocol/terms-of-service";
const ATOMGIT_PRIVACY_URL: &str = "https://docs.atomgit.com/docs/help/home/protocol/privacy-policy";
const ATOMGIT_CLA_URL: &str = "https://docs.atomgit.com/org/cla";
const ATOMGIT_GPG_URL: &str = "https://docs.atomgit.com/user/gpgkey";
const ATOMGIT_RELEASE_OVERVIEW_URL: &str =
    "https://docs.atomgit.com/docs/help/home/org_project/project_manage/release_management/release_overview/";
const ATOMGIT_RELEASE_OPERATIONS_URL: &str =
    "https://docs.atomgit.com/docs/help/home/org_project/project_manage/release_management/release_operations";
const ATOMGIT_TRADE_CONTROLS_URL: &str =
    "https://atomgit.com/site-policy/other-site-policies/atomgit-and-trade-controls";
const ATOMGIT_IP_POLICY_URL: &str =
    "https://atomgit.com/site-policy/content-removal-policies/dmca-takedown-policy";
const ATOMGIT_GOV_TAKEDOWN_URL: &str =
    "https://atomgit.com/site-policy/other-site-policies/atomgit-government-takedown-policy";
const DEFAULT_TIMEOUT_SECS: u64 = 40;

#[derive(Debug, Clone)]
struct PageSnapshot {
    http_status: Option<u16>,
    keyword_lines: Vec<String>,
    plain_text: String,
    body_fingerprint: Option<String>,
    looks_like_spa_shell: bool,
    error: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct AtomGitPlatformCollector;

impl AtomGitPlatformCollector {
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
            "atomgit" | "atomgitplatform" | "gitcode" | "gitcodeplatform"
        );
        let explicit_profile_match =
            rule_profile_key == "atomgitplatform" || rule_profile_key.contains("atomgitplatform");
        let explicit_homepage_match =
            homepage_key.contains("atomgitcom") || homepage_key.contains("docsatomgitcom");
        let is_platform_target = target_type_key == "platform";
        let platform_matches =
            platform_key == "atomgit" || platform_key == "gitcode" || explicit_profile_match;

        (explicit_name_match || explicit_profile_match || explicit_homepage_match)
            && platform_matches
            && is_platform_target
    }

    pub async fn collect(&self, target: &ecosystem_targets::Model) -> Result<Vec<Value>> {
        if !Self::matches_target(target) {
            return Ok(Vec::new());
        }

        info!("开始采集 AtomGit 平台生态目标信息");
        let client = Client::builder()
            .timeout(configured_fetch_timeout(
                "ECOSYSTEM_ATOMGIT_FETCH_TIMEOUT_SECS",
                DEFAULT_TIMEOUT_SECS,
            ))
            .user_agent("track-system/ecosystem-atomgit")
            .build()?;

        let home_page = self
            .fetch_page(
                &client,
                ATOMGIT_DOCS_HOME_URL,
                &[
                    "developer's code home",
                    "registered users",
                    "organizations teams",
                    "open source projects",
                    "code repository",
                ],
            )
            .await;
        self.log_page_result("atomgit docs home", &home_page);
        let terms_page = self
            .fetch_page(
                &client,
                ATOMGIT_TERMS_URL,
                &[
                    "版权归作者本人所有",
                    "开源许可证协议模板",
                    "atomgit可以自行决定",
                    "所有权与知识产权",
                    "非独占性使用许可",
                    "权利通知",
                ],
            )
            .await;
        self.log_page_result("atomgit terms", &terms_page);
        let privacy_page = self
            .fetch_page(
                &client,
                ATOMGIT_PRIVACY_URL,
                &[
                    "重庆开源共创科技有限公司",
                    "开放原子开源基金会",
                    "华为云计算技术有限公司",
                    "国家安全",
                    "行政机关",
                    "司法机关",
                ],
            )
            .await;
        self.log_page_result("atomgit privacy", &privacy_page);
        let cla_page = self
            .fetch_page(
                &client,
                ATOMGIT_CLA_URL,
                &["cla", "贡献者许可协议", "搜索权限", "影响范围", "管理"],
            )
            .await;
        self.log_page_result("atomgit cla", &cla_page);
        let gpg_page = self
            .fetch_page(
                &client,
                ATOMGIT_GPG_URL,
                &["gpg", "提交/tag 签名", "签名密钥", "verified", "signature"],
            )
            .await;
        self.log_page_result("atomgit gpg", &gpg_page);
        let release_overview_page = self
            .fetch_page(
                &client,
                ATOMGIT_RELEASE_OVERVIEW_URL,
                &["releases", "发行版", "软件发布列表", "里程碑", "版本追踪"],
            )
            .await;
        self.log_page_result("atomgit release overview", &release_overview_page);
        let release_operations_page = self
            .fetch_page(
                &client,
                ATOMGIT_RELEASE_OPERATIONS_URL,
                &["下载源码", "附件", "编辑发行版", "删除发行版", "发行版详情"],
            )
            .await;
        self.log_page_result("atomgit release operations", &release_operations_page);

        let random_policy_probe_url =
            format!("https://atomgit.com/__policy_probe__/{}", Uuid::new_v4());
        let random_policy_probe = self
            .fetch_page(&client, &random_policy_probe_url, &[])
            .await;
        self.log_page_result("atomgit random policy probe", &random_policy_probe);
        let trade_policy_page = self
            .fetch_page(
                &client,
                ATOMGIT_TRADE_CONTROLS_URL,
                &["trade", "control", "sanction", "管制", "制裁"],
            )
            .await;
        self.log_page_result("atomgit trade policy route", &trade_policy_page);
        let ip_policy_page = self
            .fetch_page(
                &client,
                ATOMGIT_IP_POLICY_URL,
                &["dmca", "copyright", "知识产权", "侵权", "权利通知"],
            )
            .await;
        self.log_page_result("atomgit ip policy route", &ip_policy_page);
        let gov_policy_page = self
            .fetch_page(
                &client,
                ATOMGIT_GOV_TAKEDOWN_URL,
                &["government", "takedown", "下架", "行政机关", "司法机关"],
            )
            .await;
        self.log_page_result("atomgit government takedown route", &gov_policy_page);

        Ok(self.build_evidence_records(
            home_page,
            terms_page,
            privacy_page,
            cla_page,
            gpg_page,
            release_overview_page,
            release_operations_page,
            random_policy_probe,
            trade_policy_page,
            ip_policy_page,
            gov_policy_page,
        ))
    }

    fn build_evidence_records(
        &self,
        home_page: PageSnapshot,
        terms_page: PageSnapshot,
        privacy_page: PageSnapshot,
        cla_page: PageSnapshot,
        gpg_page: PageSnapshot,
        release_overview_page: PageSnapshot,
        release_operations_page: PageSnapshot,
        random_policy_probe: PageSnapshot,
        trade_policy_page: PageSnapshot,
        ip_policy_page: PageSnapshot,
        gov_policy_page: PageSnapshot,
    ) -> Vec<Value> {
        let basic_info = self.detect_basic_info(&home_page);
        let operator_profile = self.detect_operator_profile(&privacy_page);
        let trade_controls = self.detect_trade_controls(
            &trade_policy_page,
            &random_policy_probe,
            &terms_page,
            &privacy_page,
        );
        let ip_policy = self.detect_ip_policy(&terms_page, &ip_policy_page, &random_policy_probe);
        let government_takedown = self.detect_government_takedown(
            &terms_page,
            &privacy_page,
            &gov_policy_page,
            &random_policy_probe,
        );
        let license_policy = self.detect_license_policy(&terms_page);
        let cla_policy = self.detect_cla_policy(&cla_page);
        let download_integrity = self.detect_download_integrity(
            &gpg_page,
            &release_overview_page,
            &release_operations_page,
        );
        let operator_supply_risk = self.detect_operator_supply_risk(&privacy_page, &terms_page);

        debug!(
            platform_intro = %basic_info["summary"].as_str().unwrap_or(""),
            organization_structure = %operator_profile["organization_structure"].as_str().unwrap_or(""),
            trade_controls = %trade_controls["summary"].as_str().unwrap_or(""),
            ip_policy = %ip_policy["summary"].as_str().unwrap_or(""),
            government_takedown = %government_takedown["summary"].as_str().unwrap_or(""),
            license_policy = %license_policy["summary"].as_str().unwrap_or(""),
            cla_policy = %cla_policy["summary"].as_str().unwrap_or(""),
            download_integrity = %download_integrity["summary"].as_str().unwrap_or(""),
            operator_supply_risk = %operator_supply_risk["summary"].as_str().unwrap_or(""),
            "AtomGit 平台生态目标关键信息提取完成"
        );

        vec![
            json!({
                "source_type": "atomgit_platform_overview",
                "source_name": "atomgit_platform_overview",
                "source_url": ATOMGIT_DOCS_HOME_URL,
                "assessment_category": "source",
                "assessment_subcategory": "platform_overview",
                "data": {
                    "platform_intro": basic_info["summary"],
                    "registered_user_scale": basic_info["registered_user_scale"],
                    "organization_scale": basic_info["organization_scale"],
                    "project_scale": basic_info["project_scale"],
                    "repository_scale": basic_info["repository_scale"],
                    "basic_info": basic_info["summary"],
                    "home_keyword_lines": home_page.keyword_lines,
                    "home_http_status": home_page.http_status,
                    "home_error": home_page.error,
                }
            }),
            json!({
                "source_type": "atomgit_operator_profile",
                "source_name": "atomgit_operator_profile",
                "source_url": ATOMGIT_PRIVACY_URL,
                "assessment_category": "source",
                "assessment_subcategory": "corporate_profile",
                "data": {
                    "organization_structure": operator_profile["organization_structure"],
                    "foundation_status": operator_profile["foundation_status"],
                    "operator_name": operator_profile["operator_name"],
                    "hosting_provider": operator_profile["hosting_provider"],
                    "operator_transition_date": operator_profile["operator_transition_date"],
                    "operator_transition_mentioned": operator_profile["operator_transition_mentioned"],
                    "gitcode_integration_mentioned": operator_profile["gitcode_integration_mentioned"],
                    "privacy_keyword_lines": privacy_page.keyword_lines,
                    "privacy_http_status": privacy_page.http_status,
                    "privacy_error": privacy_page.error,
                }
            }),
            json!({
                "source_type": "atomgit_trade_controls",
                "source_name": "atomgit_trade_controls",
                "source_url": ATOMGIT_TRADE_CONTROLS_URL,
                "assessment_category": "source",
                "assessment_subcategory": "trade_controls",
                "data": {
                    "trade_controls": trade_controls["summary"],
                    "trade_policy_route_reachable": trade_controls["route_reachable"],
                    "trade_policy_machine_readable": trade_controls["machine_readable_policy_text"],
                    "trade_policy_same_as_generic_spa": trade_controls["same_as_random_probe"],
                    "legal_compliance_required": trade_controls["legal_compliance_required"],
                    "trade_keyword_lines": trade_policy_page.keyword_lines,
                    "trade_http_status": trade_policy_page.http_status,
                    "trade_error": trade_policy_page.error,
                }
            }),
            json!({
                "source_type": "atomgit_ip_policy",
                "source_name": "atomgit_ip_policy",
                "source_url": ATOMGIT_TERMS_URL,
                "assessment_category": "source",
                "assessment_subcategory": "ip_policy",
                "data": {
                    "ip_policy": ip_policy["summary"],
                    "users_own_content": ip_policy["users_own_content"],
                    "license_grant_to_host_content": ip_policy["license_grant_to_host_content"],
                    "platform_retains_own_ip": ip_policy["platform_retains_own_ip"],
                    "ip_notice_process_mentioned": ip_policy["ip_notice_process_mentioned"],
                    "ip_policy_route_reachable": ip_policy["route_reachable"],
                    "ip_policy_machine_readable": ip_policy["machine_readable_policy_text"],
                    "ip_policy_same_as_generic_spa": ip_policy["same_as_random_probe"],
                    "ip_keyword_lines": ip_policy["ip_keyword_lines"],
                    "terms_http_status": terms_page.http_status,
                    "terms_error": terms_page.error,
                }
            }),
            json!({
                "source_type": "atomgit_government_takedown",
                "source_name": "atomgit_government_takedown",
                "source_url": ATOMGIT_GOV_TAKEDOWN_URL,
                "assessment_category": "source",
                "assessment_subcategory": "government_takedown_policy",
                "data": {
                    "government_takedown_policy": government_takedown["summary"],
                    "content_removal_reserved": government_takedown["content_removal_reserved"],
                    "public_authority_disclosure": government_takedown["public_authority_disclosure"],
                    "national_security_disclosure": government_takedown["national_security_disclosure"],
                    "publishes_public_requests": government_takedown["publishes_public_requests"],
                    "government_policy_route_reachable": government_takedown["route_reachable"],
                    "government_policy_machine_readable": government_takedown["machine_readable_policy_text"],
                    "government_policy_same_as_generic_spa": government_takedown["same_as_random_probe"],
                    "government_keyword_lines": government_takedown["government_keyword_lines"],
                    "government_http_status": gov_policy_page.http_status,
                    "government_error": gov_policy_page.error,
                }
            }),
            json!({
                "source_type": "atomgit_license_policy",
                "source_name": "atomgit_license_policy",
                "source_url": ATOMGIT_TERMS_URL,
                "assessment_category": "source",
                "assessment_subcategory": "license_policy",
                "data": {
                    "license_policy": license_policy["summary"],
                    "supports_license_templates": license_policy["supports_license_templates"],
                    "license_remains_with_author": license_policy["license_remains_with_author"],
                    "license_keyword_lines": license_policy["license_keyword_lines"],
                    "license_http_status": terms_page.http_status,
                    "license_error": terms_page.error,
                }
            }),
            json!({
                "source_type": "atomgit_cla_policy",
                "source_name": "atomgit_cla_policy",
                "source_url": ATOMGIT_CLA_URL,
                "assessment_category": "source",
                "assessment_subcategory": "cla_policy",
                "data": {
                    "cla_policy": cla_policy["summary"],
                    "cla_supported": cla_policy["cla_supported"],
                    "cla_management_scope": cla_policy["cla_management_scope"],
                    "cla_keyword_lines": cla_policy["cla_keyword_lines"],
                    "cla_http_status": cla_page.http_status,
                    "cla_error": cla_page.error,
                }
            }),
            json!({
                "source_type": "atomgit_operator_supply_risk",
                "source_name": "atomgit_operator_supply_risk",
                "source_url": ATOMGIT_PRIVACY_URL,
                "assessment_category": "source",
                "assessment_subcategory": "operator_supply_risk",
                "data": {
                    "operator_supply_risk": operator_supply_risk["summary"],
                    "operator_supply_risk_level": operator_supply_risk["risk_level"],
                    "single_operator_concentration": operator_supply_risk["single_operator_concentration"],
                    "cloud_vendor_dependency": operator_supply_risk["cloud_vendor_dependency"],
                    "operator_transition_risk": operator_supply_risk["operator_transition_risk"],
                    "delegation_transfer_clause": operator_supply_risk["delegation_transfer_clause"],
                }
            }),
            json!({
                "source_type": "atomgit_download_integrity",
                "source_name": "atomgit_download_integrity",
                "source_url": ATOMGIT_GPG_URL,
                "assessment_category": "quality",
                "assessment_subcategory": "release_quality",
                "data": {
                    "hash_signature_assessment": download_integrity["summary"],
                    "hash_verification_supported": download_integrity["hash_verification_supported"],
                    "digital_signature_supported": download_integrity["digital_signature_supported"],
                    "supports_gpg_commit_tag_verification": download_integrity["supports_gpg_commit_tag_verification"],
                    "supports_release_attachments": download_integrity["supports_release_attachments"],
                    "documented_release_checksum": download_integrity["documented_release_checksum"],
                    "documented_release_artifact_signature": download_integrity["documented_release_artifact_signature"],
                    "signed_releases": download_integrity["signed_releases"],
                    "provenance_attestation": download_integrity["provenance_attestation"],
                    "gpg_keyword_lines": gpg_page.keyword_lines,
                    "release_keyword_lines": release_operations_page.keyword_lines,
                    "gpg_http_status": gpg_page.http_status,
                    "release_http_status": release_operations_page.http_status,
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
                            body_fingerprint: Some(sha256_hex(plain_text.as_bytes())),
                            looks_like_spa_shell: looks_like_spa_shell(&body),
                            plain_text,
                            error: None,
                        }
                    }
                    Err(error) => PageSnapshot {
                        http_status: Some(status),
                        keyword_lines: Vec::new(),
                        plain_text: String::new(),
                        body_fingerprint: None,
                        looks_like_spa_shell: false,
                        error: Some(error.to_string()),
                    },
                }
            }
            Err(error) => PageSnapshot {
                http_status: None,
                keyword_lines: Vec::new(),
                plain_text: String::new(),
                body_fingerprint: None,
                looks_like_spa_shell: false,
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
                "AtomGit 页面抓取失败"
            ),
            None => info!(
                page = label,
                http_status = ?page.http_status,
                spa_shell = page.looks_like_spa_shell,
                keyword_lines = ?page.keyword_lines,
                "AtomGit 页面抓取成功"
            ),
        }
        debug!(
            page = label,
            plain_text_preview = %page.plain_text.chars().take(240).collect::<String>(),
            "AtomGit 页面文本预览"
        );
    }

    fn detect_basic_info(&self, home_page: &PageSnapshot) -> Value {
        let text = home_page.plain_text.as_str();
        let lower = text.to_ascii_lowercase();
        let registered_user_scale = extract_metric_with_phrase(text, "Registered Users");
        let organization_scale = extract_metric_with_phrase(text, "Organizations Teams");
        let project_scale = extract_metric_with_phrase(text, "Open Source Projects");
        let repository_scale = extract_metric_with_phrase(text, "Code Repository");
        let has_platform_intro = lower.contains("developer's code home")
            || lower.contains("open source community")
            || lower.contains("software development platform");

        let summary = if has_platform_intro {
            format!(
                "AtomGit 是面向开发者的开源社区与代码托管协作平台{}{}{}{}",
                registered_user_scale
                    .as_ref()
                    .map(|v| format!("，公开页面提及 {} Registered Users", v))
                    .unwrap_or_default(),
                organization_scale
                    .as_ref()
                    .map(|v| format!("、{} Organizations Teams", v))
                    .unwrap_or_default(),
                project_scale
                    .as_ref()
                    .map(|v| format!("、{} Open Source Projects", v))
                    .unwrap_or_default(),
                repository_scale
                    .as_ref()
                    .map(|v| format!("、{} Code Repository", v))
                    .unwrap_or_default(),
            )
        } else {
            "AtomGit 是提供代码托管、协作开发、项目管理与发布能力的平台".to_string()
        };

        json!({
            "summary": summary,
            "registered_user_scale": registered_user_scale,
            "organization_scale": organization_scale,
            "project_scale": project_scale,
            "repository_scale": repository_scale,
        })
    }

    fn detect_operator_profile(&self, privacy_page: &PageSnapshot) -> Value {
        let text = privacy_page.plain_text.as_str();
        let operator_transition_mentioned = text
            .contains("开放原子开源基金会变更为重庆开源共创科技有限公司")
            || text.contains("2025 年 9 月 9 日");
        let gitcode_integration_mentioned =
            text.contains("GitCode 平台用户体系、产品体系、运营体系和客服体系");
        let operator_name = if text.contains("重庆开源共创科技有限公司") {
            Some("重庆开源共创科技有限公司".to_string())
        } else {
            None
        };
        let hosting_provider = if text.contains("华为云计算技术有限公司") {
            Some("华为云计算技术有限公司".to_string())
        } else {
            None
        };

        let organization_structure = if operator_transition_mentioned {
            "AtomGit 当前由重庆开源共创科技有限公司运营，并与 GitCode 平台体系深度融合，整体属于商业公司主导的平台治理模式".to_string()
        } else {
            "AtomGit 更接近公司化平台运营模式，治理结构由平台运营团队与服务体系主导".to_string()
        };

        let foundation_status = if operator_transition_mentioned {
            "隐私政策明确自 2025-09-09 起运营主体由开放原子开源基金会变更为重庆开源共创科技有限公司，当前不属于基金会直接独立运营形态".to_string()
        } else {
            "未从当前公开页面识别出基金会直接治理安排，整体更接近公司运营".to_string()
        };

        json!({
            "organization_structure": organization_structure,
            "foundation_status": foundation_status,
            "operator_name": operator_name,
            "hosting_provider": hosting_provider,
            "operator_transition_date": if operator_transition_mentioned {
                Some("2025-09-09".to_string())
            } else {
                None::<String>
            },
            "operator_transition_mentioned": operator_transition_mentioned,
            "gitcode_integration_mentioned": gitcode_integration_mentioned,
        })
    }

    fn detect_trade_controls(
        &self,
        trade_policy_page: &PageSnapshot,
        random_policy_probe: &PageSnapshot,
        terms_page: &PageSnapshot,
        privacy_page: &PageSnapshot,
    ) -> Value {
        let route_reachable = is_reachable(trade_policy_page);
        let same_as_random_probe =
            is_same_shell_as_random_probe(trade_policy_page, random_policy_probe);
        let machine_readable_policy_text = route_reachable
            && !trade_policy_page.looks_like_spa_shell
            && !trade_policy_page.keyword_lines.is_empty();
        let legal_compliance_required = terms_page.plain_text.contains("中华人民共和国")
            || terms_page.plain_text.contains("法律法规")
            || privacy_page.plain_text.contains("法律法规")
            || privacy_page.plain_text.contains("行政机关");

        let summary = if machine_readable_policy_text {
            "AtomGit 公开提供了可读的贸易/制裁政策页面，平台受合规约束并可能对相关访问或内容实施限制".to_string()
        } else if route_reachable {
            "AtomGit 官方存在贸易管制政策路由，但当前返回统一 SPA 壳页，未检索到可读的具体条款；结合条款与隐私政策，可确认平台受中国法律法规、内容治理与行政司法要求约束，无法证明其不受贸易/合规限制影响".to_string()
        } else {
            "暂未获取到可读的 AtomGit 贸易管制公开页面；结合条款与隐私政策，平台仍受法律法规和行政司法要求约束，无法证明其不受贸易/合规限制影响".to_string()
        };

        json!({
            "summary": summary,
            "route_reachable": route_reachable,
            "machine_readable_policy_text": machine_readable_policy_text,
            "same_as_random_probe": same_as_random_probe,
            "legal_compliance_required": legal_compliance_required,
        })
    }

    fn detect_ip_policy(
        &self,
        terms_page: &PageSnapshot,
        ip_policy_page: &PageSnapshot,
        random_policy_probe: &PageSnapshot,
    ) -> Value {
        let text = terms_page.plain_text.as_str();
        let users_own_content = text.contains("版权归作者本人所有");
        let license_grant_to_host_content = text.contains("AtomGit可以自行决定以全部或任何方式")
            || text.contains("非独占性使用许可");
        let platform_retains_own_ip = text.contains("与AtomGit服务相关的知识产权")
            || text.contains("所有的程序及页面内容均受版权法保护");
        let ip_notice_process_mentioned =
            text.contains("权利通知") && text.contains("知识产权声明");
        let route_reachable = is_reachable(ip_policy_page);
        let same_as_random_probe =
            is_same_shell_as_random_probe(ip_policy_page, random_policy_probe);
        let machine_readable_policy_text = route_reachable
            && !ip_policy_page.looks_like_spa_shell
            && !ip_policy_page.keyword_lines.is_empty();

        let summary = format!(
            "AtomGit 条款明确{}{}{}{}",
            if users_own_content {
                "用户上传内容版权归作者本人所有"
            } else {
                "用户内容权属边界需要进一步结合条款确认"
            },
            if license_grant_to_host_content {
                "，发布内容时需向平台授予非独占使用许可"
            } else {
                ""
            },
            if platform_retains_own_ip {
                "；平台程序、页面与服务相关知识产权由平台保留"
            } else {
                ""
            },
            if ip_notice_process_mentioned {
                "，并提供权利通知/知识产权声明处理机制"
            } else if route_reachable && !machine_readable_policy_text {
                "；知识产权政策路由可达，但当前未提供可读正文"
            } else {
                ""
            }
        );

        json!({
            "summary": summary,
            "users_own_content": users_own_content,
            "license_grant_to_host_content": license_grant_to_host_content,
            "platform_retains_own_ip": platform_retains_own_ip,
            "ip_notice_process_mentioned": ip_notice_process_mentioned,
            "route_reachable": route_reachable,
            "machine_readable_policy_text": machine_readable_policy_text,
            "same_as_random_probe": same_as_random_probe,
            "ip_keyword_lines": extract_keyword_lines(
                text,
                &["版权归作者本人所有", "非独占性使用许可", "知识产权", "权利通知", "版权法保护"],
                10
            ),
        })
