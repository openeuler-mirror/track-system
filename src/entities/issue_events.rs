/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. ctscat is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

use sea_orm::{entity::prelude::*, JsonValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "issue_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub issue_id: i32,
    pub event_type: String,
    pub actor: Option<String>,
    pub event_at: DateTimeUtc,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub payload: Option<JsonValue>,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::issues::Entity",
        from = "Column::IssueId",
        to = "super::issues::Column::Id",
        on_delete = "Cascade",
        on_update = "NoAction"
    )]
    Issues,
}

impl Related<super::issues::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Issues.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
