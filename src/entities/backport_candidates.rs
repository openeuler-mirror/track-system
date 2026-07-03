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

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "backport_candidates")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub package_id: i32,
    pub l0_commit_id: i64,
    pub target_distro_id: i32,
    pub spec_base_version: String,
    #[sea_orm(column_type = "Text")]
    pub recommendation: String,
    pub status: String,
    pub patch_artifact: Option<String>,
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
    #[sea_orm(
        belongs_to = "super::l0_commits::Entity",
        from = "Column::L0CommitId",
        to = "super::l0_commits::Column::Id",
        on_delete = "Cascade",
        on_update = "NoAction"
    )]
    L0Commits,
    #[sea_orm(
        belongs_to = "super::distros::Entity",
        from = "Column::TargetDistroId",
        to = "super::distros::Column::Id",
        on_delete = "Cascade",
        on_update = "NoAction"
    )]
    Distros,
}

impl Related<super::packages::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Packages.def()
    }
}

impl Related<super::l0_commits::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::L0Commits.def()
    }
}

impl Related<super::distros::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Distros.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
