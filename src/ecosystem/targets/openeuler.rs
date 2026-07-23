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
