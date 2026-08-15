use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebPageSnapshot {
    pub url: String,
    pub http_status: Option<i32>,
    pub body_hash: Option<String>,
    pub content: Option<String>,
}
