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

use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Tracking::Table)
                    .if_not_exists()
                    .col(pk_auto(Tracking::Id))
                    .col(integer(Tracking::PackageId))
                    .col(integer(Tracking::DistroId))
                    .col(string(Tracking::L1Branch))
                    .col(string(Tracking::L1RepoOwner))
                    .col(string(Tracking::L1RepoName))
                    .col(string(Tracking::L2Branch))
                    .col(string(Tracking::L2RepoPath))
                    .col(string(Tracking::TrackingStatus))
                    .col(timestamp_null(Tracking::LastSyncTime))
                    .col(string_null(Tracking::LastL1CommitSha))
                    .col(string_null(Tracking::LastL2CommitSha))
                    .col(timestamp(Tracking::CreatedAt))
                    .col(timestamp(Tracking::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tracking_package")
                            .from(Tracking::Table, Tracking::PackageId)
                            .to(Packages::Table, Packages::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tracking_distro")
                            .from(Tracking::Table, Tracking::DistroId)
                            .to(Distros::Table, Distros::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tracking_package")
                    .table(Tracking::Table)
                    .col(Tracking::PackageId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tracking_distro")
                    .table(Tracking::Table)
                    .col(Tracking::DistroId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tracking_status")
                    .table(Tracking::Table)
                    .col(Tracking::TrackingStatus)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Tracking::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
#[allow(clippy::enum_variant_names)]
enum Tracking {
    Table,
    Id,
    PackageId,
    DistroId,
    L1Branch,
    L1RepoOwner,
    L1RepoName,
    L2Branch,
    L2RepoPath,
    TrackingStatus,
    LastSyncTime,
    LastL1CommitSha,
    LastL2CommitSha,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Packages {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Distros {
    Table,
    Id,
}
