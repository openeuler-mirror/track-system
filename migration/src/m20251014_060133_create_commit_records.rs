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
                    .table(CommitRecords::Table)
                    .if_not_exists()
                    .col(pk_auto(CommitRecords::Id))
                    .col(integer(CommitRecords::TrackingId))
                    .col(string(CommitRecords::CommitSha))
                    .col(text(CommitRecords::CommitMessage))
                    .col(string(CommitRecords::AuthorName))
                    .col(string(CommitRecords::AuthorEmail))
                    .col(timestamp(CommitRecords::CommittedAt))
                    .col(string_null(CommitRecords::ChangeType))
                    .col(string(CommitRecords::SyncStatus))
                    .col(string_null(CommitRecords::SyncedToL2Commit))
                    .col(timestamp_null(CommitRecords::SyncedAt))
