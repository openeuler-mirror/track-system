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
#[sea_orm(table_name = "compare_reports")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub tracking_id: i32,
    pub generated_at: DateTimeUtc,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub l2_vs_l1_diff: Option<JsonValue>,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub l1_vs_l0_diff: Option<JsonValue>,
    pub status: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub failure_reason: Option<String>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::tracking::Entity",
        from = "Column::TrackingId",
        to = "super::tracking::Column::Id",
        on_delete = "Cascade",
        on_update = "NoAction"
    )]
    Tracking,
}

impl Related<super::tracking::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tracking.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
