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
