use anyhow::Result;
use serde_json::{json, Value};

use crate::entities::ecosystem_targets;

#[derive(Debug, Default, Clone)]
pub struct GitHubEcosystemCollector;

impl GitHubEcosystemCollector {
    pub fn new() -> Self {
        Self
    }

    pub async fn collect(&self, target: &ecosystem_targets::Model) -> Result<Vec<Value>> {
        let repo_ref = format!(
            "{}/{}",
            target
                .owner
                .clone()
                .unwrap_or_else(|| "unknown-owner".to_string()),
            target.repo.clone().unwrap_or_else(|| target.name.clone())
        );

        Ok(vec![
            json!({
                "source_type": "github_platform_profile",
                "source_name": "github_platform",
                "source_url": target.homepage_url.clone().unwrap_or_else(|| "https://github.com".to_string()),
                "assessment_category": "source",
                "assessment_subcategory": "hosting_platform",
                "data": {
