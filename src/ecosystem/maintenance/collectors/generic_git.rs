use anyhow::{anyhow, Context, Result};
use chrono::{TimeZone, Utc};
use git2::{BranchType, Oid, Repository, Sort};
use regex::Regex;
use reqwest::Url;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};

use crate::entities::packages;
use crate::utils::version::{Version, VersionParser};

const GENERIC_GIT_CACHE_ENV: &str = "TRACK_SYSTEM_GENERIC_GIT_CACHE_DIR";
const GENERIC_GIT_CACHE_RETENTION_ENV: &str = "TRACK_SYSTEM_GENERIC_GIT_CACHE_RETENTION_ENABLED";
