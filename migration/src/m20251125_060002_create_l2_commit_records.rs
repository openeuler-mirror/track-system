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
                    .table(L2CommitRecords::Table)
                    .if_not_exists()
                    .col(pk_auto(L2CommitRecords::Id))
                    .col(integer(L2CommitRecords::TrackingId))
                    .col(string(L2CommitRecords::CommitSha))
                    .col(text(L2CommitRecords::CommitMessage))
                    .col(string(L2CommitRecords::AuthorName))
                    .col(string(L2CommitRecords::AuthorEmail))
                    .col(timestamp(L2CommitRecords::CommittedAt))
                    .col(string_null(L2CommitRecords::ChangeType))
                    .col(string_null(L2CommitRecords::PrimaryChangeType))
                    .col(binary_null(L2CommitRecords::CveList))
                    .col(boolean(L2CommitRecords::SpecChanged).default(false))
                    .col(binary_null(L2CommitRecords::PatchStats))
                    .col(string(L2CommitRecords::ClassificationStatus).default("pending"))
                    .col(text_null(L2CommitRecords::ClassificationNotes))
                    .col(string(L2CommitRecords::SyncStatus))
