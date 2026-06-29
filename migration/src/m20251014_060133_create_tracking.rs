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
