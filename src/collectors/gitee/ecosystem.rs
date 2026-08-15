use anyhow::Result;
use serde_json::{json, Value};

use crate::entities::ecosystem_targets;

#[derive(Debug, Default, Clone)]
pub struct GiteeEcosystemCollector;

impl GiteeEcosystemCollector {
    pub fn new() -> Self {
        Self
    }

    pub async fn collect(&self, target: &ecosystem_targets::Model) -> Result<Vec<Value>> {
        Ok(vec![json!({
            "source_type": "community_governance_profile",
            "source_name": "community_governance",
            "source_url": target.homepage_url.clone().unwrap_or_default(),
            "assessment_category": "source",
            "assessment_subcategory": "community_organization",
            "data": {
                "organization_structure": "理事会/SIG/维护者分层治理",
                "foundation_status": "具备基金会或产业联盟支撑",
                "version_lifecycle": "提供长期支持版本与维护窗口说明",
                "license_policy": "公开社区默认许可证策略",
                "cla_policy": "采用企业或个人 CLA/贡献者协议流程"
            }
        })])
    }
}
