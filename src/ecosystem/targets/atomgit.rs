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
