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

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuditLogs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuditLogs::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AuditLogs::UserId).string().null())
                    .col(ColumnDef::new(AuditLogs::Action).string().not_null())
                    .col(ColumnDef::new(AuditLogs::ResourceType).string().not_null())
                    .col(ColumnDef::new(AuditLogs::ResourceId).string().null())
                    .col(ColumnDef::new(AuditLogs::Method).string().not_null())
                    .col(ColumnDef::new(AuditLogs::Path).string().not_null())
                    .col(ColumnDef::new(AuditLogs::IpAddress).string().null())
                    .col(ColumnDef::new(AuditLogs::UserAgent).text().null())
                    .col(ColumnDef::new(AuditLogs::RequestBody).json().null())
                    .col(
                        ColumnDef::new(AuditLogs::ResponseStatus)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(AuditLogs::ResponseBody).json().null())
                    .col(ColumnDef::new(AuditLogs::Duration).integer().null())
                    .col(ColumnDef::new(AuditLogs::ErrorMessage).text().null())
                    .col(
                        ColumnDef::new(AuditLogs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建索引以提高查询性能
        manager
            .create_index(
                Index::create()
                    .name("idx_audit_logs_user_id")
                    .table(AuditLogs::Table)
                    .col(AuditLogs::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_audit_logs_action")
                    .table(AuditLogs::Table)
                    .col(AuditLogs::Action)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_audit_logs_resource")
                    .table(AuditLogs::Table)
                    .col(AuditLogs::ResourceType)
                    .col(AuditLogs::ResourceId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_audit_logs_created_at")
                    .table(AuditLogs::Table)
                    .col(AuditLogs::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuditLogs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AuditLogs {
    Table,
    Id,
    UserId,
    Action,
    ResourceType,
    ResourceId,
    Method,
    Path,
    IpAddress,
    UserAgent,
    RequestBody,
    ResponseStatus,
    ResponseBody,
    Duration,
    ErrorMessage,
    CreatedAt,
}
