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
