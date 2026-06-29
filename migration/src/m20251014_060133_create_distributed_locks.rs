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
                    .table(DistributedLocks::Table)
                    .if_not_exists()
                    .col(pk_auto(DistributedLocks::Id))
                    .col(string_uniq(DistributedLocks::LockKey))
                    .col(string(DistributedLocks::Owner))
                    .col(timestamp(DistributedLocks::AcquiredAt))
                    .col(timestamp(DistributedLocks::ExpiresAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_distributed_locks_expires")
                    .table(DistributedLocks::Table)
                    .col(DistributedLocks::ExpiresAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(DistributedLocks::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum DistributedLocks {
    Table,
    Id,
    LockKey,
    Owner,
    AcquiredAt,
    ExpiresAt,
}
