use anyhow::Result;
use serde_json::{json, Value};

use crate::entities::ecosystem_targets;

#[derive(Debug, Default, Clone)]
pub struct AtomGitEcosystemCollector;

impl AtomGitEcosystemCollector {
    pub fn new() -> Self {
        Self
    }

    pub async fn collect(&self, target: &ecosystem_targets::Model) -> Result<Vec<Value>> {
        Ok(vec![json!({
            "source_type": "atomgit_platform_profile",
            "source_name": "atomgit_platform",
            "source_url": target.homepage_url.clone().unwrap_or_else(|| "https://atomgit.com".to_string()),
            "assessment_category": "source",
            "assessment_subcategory": "download_platform",
            "data": {
                "basic_info": format!("AtomGit 组件下载页 {}", target.name),
                "trade_controls": "平台需遵循所在司法辖区的合规与出口要求",
                "ip_policy": "提供知识产权声明、侵权投诉与内容处置入口",
                "government_takedown_policy": "存在监管、投诉或法务驱动的下架机制",
                "license_policy": "支持展示组件许可证与仓库授权信息",
                "cla_policy": "可结合平台工作流或外部系统维护 CLA 状态"
            }
        })])
    }
}
