use anyhow::Result;

use super::models::WebPageSnapshot;

#[derive(Debug, Default, Clone)]
pub struct WebPageCollector;

impl WebPageCollector {
    pub fn new() -> Self {
        Self
    }

    pub async fn fetch(&self, url: &str) -> Result<WebPageSnapshot> {
        Ok(WebPageSnapshot {
            url: url.to_string(),
            http_status: None,
            body_hash: None,
            content: None,
        })
    }
}
