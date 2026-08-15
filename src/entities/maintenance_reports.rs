use sea_orm::{entity::prelude::*, JsonValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "maintenance_reports")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub package_id: i32,
    pub report_type: String,
    pub status: String,
    pub overall_risk: String,
    pub confidence: String,
    #[sea_orm(column_type = "Text")]
    pub summary: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub dimensions: JsonValue,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub evidence_summary: Option<JsonValue>,
    #[sea_orm(column_type = "JsonBinary")]
    pub report_payload: JsonValue,
    pub generated_at: DateTimeUtc,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::packages::Entity",
        from = "Column::PackageId",
        to = "super::packages::Column::Id",
        on_delete = "Cascade",
        on_update = "NoAction"
    )]
    Packages,
}

impl Related<super::packages::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Packages.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
