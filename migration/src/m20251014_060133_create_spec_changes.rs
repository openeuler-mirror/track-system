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
                    .table(SpecChanges::Table)
                    .if_not_exists()
                    .col(pk_auto(SpecChanges::Id))
                    .col(integer(SpecChanges::CommitRecordId))
                    .col(integer_null(SpecChanges::OldSnapshotId))
                    .col(integer(SpecChanges::NewSnapshotId))
                    .col(boolean(SpecChanges::VersionChanged))
                    .col(string_null(SpecChanges::OldVersion))
                    .col(string_null(SpecChanges::NewVersion))
                    .col(boolean(SpecChanges::ReleaseChanged))
                    .col(string_null(SpecChanges::OldRelease))
                    .col(string_null(SpecChanges::NewRelease))
                    .col(boolean(SpecChanges::SourcesChanged))
                    .col(integer(SpecChanges::SourcesAdded))
                    .col(integer(SpecChanges::SourcesRemoved))
                    .col(integer(SpecChanges::SourcesModified))
                    .col(boolean(SpecChanges::PatchesChanged))
                    .col(integer(SpecChanges::PatchesAdded))
                    .col(integer(SpecChanges::PatchesRemoved))
                    .col(integer(SpecChanges::PatchesModified))
                    .col(integer(SpecChanges::ChangelogEntriesAdded))
                    .col(timestamp(SpecChanges::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_spec_changes_commit")
                            .from(SpecChanges::Table, SpecChanges::CommitRecordId)
                            .to(CommitRecords::Table, CommitRecords::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_spec_changes_old")
                            .from(SpecChanges::Table, SpecChanges::OldSnapshotId)
                            .to(SpecSnapshots::Table, SpecSnapshots::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_spec_changes_new")
                            .from(SpecChanges::Table, SpecChanges::NewSnapshotId)
                            .to(SpecSnapshots::Table, SpecSnapshots::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_spec_changes_commit")
                    .table(SpecChanges::Table)
                    .col(SpecChanges::CommitRecordId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SpecChanges::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum SpecChanges {
    Table,
    Id,
    CommitRecordId,
    OldSnapshotId,
    NewSnapshotId,
    VersionChanged,
    OldVersion,
    NewVersion,
    ReleaseChanged,
    OldRelease,
    NewRelease,
    SourcesChanged,
    SourcesAdded,
    SourcesRemoved,
    SourcesModified,
    PatchesChanged,
    PatchesAdded,
    PatchesRemoved,
    PatchesModified,
    ChangelogEntriesAdded,
    CreatedAt,
}

#[derive(DeriveIden)]
enum CommitRecords {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum SpecSnapshots {
    Table,
    Id,
}
