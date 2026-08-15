use sea_orm::{entity::prelude::*, JsonValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "ecosystem_bindings")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub target_id: i32,
    pub bind_type: String,
    pub bind_id: Option<i32>,
    pub relation_role: String,
    pub is_primary: bool,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub metadata: Option<JsonValue>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::ecosystem_targets::Entity",
        from = "Column::TargetId",
        to = "super::ecosystem_targets::Column::Id",
        on_delete = "Cascade",
        on_update = "NoAction"
    )]
    EcosystemTargets,
}

impl Related<super::ecosystem_targets::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EcosystemTargets.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
