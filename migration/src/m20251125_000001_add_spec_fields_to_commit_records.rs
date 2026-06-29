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

use sea_orm_migration::{prelude::*, sea_query::TableAlterStatement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 为 commit_records 表添加 spec_version 与 spec_release 字段（SQLite 不支持一次多项 ALTER，拆分执行）
        let add_spec_version: TableAlterStatement = Table::alter()
            .table(CommitRecords::Table)
            .add_column(ColumnDef::new(CommitRecords::SpecVersion).string().null())
            .to_owned();

        manager.alter_table(add_spec_version).await?;

        let add_spec_release: TableAlterStatement = Table::alter()
            .table(CommitRecords::Table)
            .add_column(ColumnDef::new(CommitRecords::SpecRelease).string().null())
            .to_owned();

        manager.alter_table(add_spec_release).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 回滚：移除新增字段（SQLite 不支持一次多项 ALTER，拆分执行）
        let drop_spec_release: TableAlterStatement = Table::alter()
            .table(CommitRecords::Table)
            .drop_column(CommitRecords::SpecRelease)
            .to_owned();
        manager.alter_table(drop_spec_release).await?;

        let drop_spec_version: TableAlterStatement = Table::alter()
            .table(CommitRecords::Table)
            .drop_column(CommitRecords::SpecVersion)
            .to_owned();
        manager.alter_table(drop_spec_version).await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
#[allow(dead_code)]
enum CommitRecords {
    Table,
    Id,
    TrackingId,
    CommitSha,
    CommitMessage,
    AuthorName,
    AuthorEmail,
    CommittedAt,
    ChangeType,
    SyncStatus,
    SyncedToL2Commit,
    SyncedAt,
    ApiUrl,
    FetchedAt,
    FilesChangedCount,
    Additions,
    Deletions,
    CreatedAt,
    UpdatedAt,
    // 新增列
    SpecVersion,
    SpecRelease,
}
