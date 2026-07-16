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
                    "basic_info": format!("GitHub 仓库 {}", repo_ref),
                    "trade_controls": "平台位于美国，需关注出口管制与服务可达性策略",
                    "ip_policy": "提供 DMCA/知识产权投诉通道与内容处置流程",
                    "government_takedown_policy": "存在政府或合规驱动的仓库限制与下架机制",
                    "license_policy": "支持 SPDX License 展示与 License API",
                    "cla_policy": "支持通过 CLA Bot 或仓库流程接入贡献者协议"
                }
            }),
            json!({
                "source_type": "github_component_community",
                "source_name": "component_community",
                "source_url": target.homepage_url.clone().unwrap_or_default(),
                "assessment_category": "source",
                "assessment_subcategory": "component_community",
                "data": {
                    "top_contributors": [
                        {"login": "maintainer-a", "commits": 182},
                        {"login": "maintainer-b", "commits": 131},
                        {"login": "maintainer-c", "commits": 96}
                    ],
                    "foundation_list": ["OpenSSF", "CNCF Security TAG"],
                    "donor_countries": ["CN", "DE", "US"]
                }
            }),
            json!({
                "source_type": "github_repository_activity",
                "source_name": "github_repository_activity",
                "source_url": target.homepage_url.clone().unwrap_or_default(),
                "assessment_category": "maintenance",
                "assessment_subcategory": "repository_activity",
                "data": {
