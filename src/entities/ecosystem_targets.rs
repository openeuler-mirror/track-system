use sea_orm::{entity::prelude::*, JsonValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "ecosystem_targets")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub target_type: String,
    pub platform: Option<String>,
    pub role: String,
    pub homepage_url: Option<String>,
    pub api_base_url: Option<String>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub default_branch: Option<String>,
    pub status: String,
    pub refresh_interval_hours: i32,
    pub rule_profile: String,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub metadata: Option<JsonValue>,
    pub last_collected_at: Option<DateTimeUtc>,
    pub last_report_at: Option<DateTimeUtc>,
    #[sea_orm(column_type = "Text", nullable)]
    pub last_error: Option<String>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::ecosystem_bindings::Entity")]
    EcosystemBindings,
    #[sea_orm(has_many = "super::ecosystem_evidence_snapshots::Entity")]
    EcosystemEvidenceSnapshots,
    #[sea_orm(has_many = "super::ecosystem_reports::Entity")]
    EcosystemReports,
}

impl Related<super::ecosystem_bindings::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EcosystemBindings.def()
    }
}

impl Related<super::ecosystem_evidence_snapshots::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EcosystemEvidenceSnapshots.def()
    }
}

impl Related<super::ecosystem_reports::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EcosystemReports.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
