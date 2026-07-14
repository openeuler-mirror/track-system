use sea_orm::{entity::prelude::*, JsonValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "maintenance_evidence_snapshots")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub package_id: i32,
    pub source_type: String,
    pub source_name: String,
    #[sea_orm(column_type = "Text")]
    pub source_url: String,
    pub http_status: Option<i32>,
    pub content_hash: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub raw_payload: JsonValue,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub normalized_signals: Option<JsonValue>,
    pub collected_at: DateTimeUtc,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
