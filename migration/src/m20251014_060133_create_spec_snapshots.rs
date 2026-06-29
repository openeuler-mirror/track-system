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
                    .table(SpecSnapshots::Table)
                    .if_not_exists()
                    .col(pk_auto(SpecSnapshots::Id))
                    .col(integer(SpecSnapshots::CommitRecordId))
                    .col(string(SpecSnapshots::SpecFilename))
                    .col(string(SpecSnapshots::Name))
                    .col(string(SpecSnapshots::Version))
                    .col(string(SpecSnapshots::Release))
                    .col(text_null(SpecSnapshots::Summary))
                    .col(string_null(SpecSnapshots::License))
                    .col(text(SpecSnapshots::Sources))
                    .col(text(SpecSnapshots::Patches))
                    .col(text_null(SpecSnapshots::LatestChangelogEntry))
                    .col(text(SpecSnapshots::FullContent))
                    .col(string(SpecSnapshots::ContentHash))
                    .col(text(SpecSnapshots::DownloadUrl))
                    .col(timestamp(SpecSnapshots::FetchedAt))
                    .col(timestamp(SpecSnapshots::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_spec_commit")
                            .from(SpecSnapshots::Table, SpecSnapshots::CommitRecordId)
                            .to(CommitRecords::Table, CommitRecords::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_spec_commit")
                    .table(SpecSnapshots::Table)
                    .col(SpecSnapshots::CommitRecordId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_spec_version")
                    .table(SpecSnapshots::Table)
                    .col(SpecSnapshots::Name)
                    .col(SpecSnapshots::Version)
                    .col(SpecSnapshots::Release)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_spec_hash")
                    .table(SpecSnapshots::Table)
