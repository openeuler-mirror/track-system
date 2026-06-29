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
                    .table(Packages::Table)
                    .if_not_exists()
                    .col(pk_auto(Packages::Id))
                    .col(string(Packages::Name).unique_key())
                    .col(integer(Packages::Level))
                    .col(integer(Packages::SyncIntervalHours))
                    .col(string_null(Packages::L0RepoUrl))
                    .col(string_null(Packages::Description))
                    .col(timestamp(Packages::CreatedAt))
                    .col(timestamp(Packages::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        // 创建索引
        manager
