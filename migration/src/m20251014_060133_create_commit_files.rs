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
                    .table(CommitFiles::Table)
                    .if_not_exists()
                    .col(pk_auto(CommitFiles::Id))
                    .col(integer(CommitFiles::CommitRecordId))
                    .col(string(CommitFiles::Filename))
                    .col(text(CommitFiles::FilePath))
                    .col(string(CommitFiles::ChangeType))
                    .col(integer(CommitFiles::Additions))
                    .col(integer(CommitFiles::Deletions))
                    .col(text_null(CommitFiles::PatchUrl))
                    .col(boolean(CommitFiles::IsSpec))
                    .col(boolean(CommitFiles::IsPatch))
                    .col(timestamp(CommitFiles::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_commit_files_commit")
                            .from(CommitFiles::Table, CommitFiles::CommitRecordId)
                            .to(CommitRecords::Table, CommitRecords::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_commit_files_commit")
                    .table(CommitFiles::Table)
                    .col(CommitFiles::CommitRecordId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_commit_files_spec")
                    .table(CommitFiles::Table)
                    .col(CommitFiles::IsSpec)
                    .to_owned(),
