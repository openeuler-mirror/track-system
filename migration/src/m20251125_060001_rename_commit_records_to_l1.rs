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

use sea_orm::{DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();

        // SQLite 不支持直接重命名表，需要使用 ALTER TABLE RENAME TO
        // MySQL 和 PostgreSQL 支持 RENAME TABLE/ALTER TABLE RENAME

        match backend {
            DatabaseBackend::Sqlite => {
                // SQLite: ALTER TABLE old_name RENAME TO new_name
                let statements = vec!["ALTER TABLE commit_records RENAME TO l1_commit_records"];

                for sql in statements {
                    manager
                        .get_connection()
                        .execute(Statement::from_string(backend, sql.to_string()))
                        .await?;
                }
            }
            DatabaseBackend::Postgres => {
                // PostgreSQL: 重命名表、索引和约束
                let statements = vec![
