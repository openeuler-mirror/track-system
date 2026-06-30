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
                    "ALTER TABLE commit_records RENAME TO l1_commit_records",
                    "ALTER INDEX idx_commit_tracking RENAME TO idx_l1_commit_tracking",
                    "ALTER INDEX idx_commit_sha RENAME TO idx_l1_commit_sha",
                    "ALTER INDEX idx_commit_status RENAME TO idx_l1_commit_status",
                    "ALTER INDEX idx_commit_type RENAME TO idx_l1_commit_type",
                    "ALTER INDEX idx_commit_tracking_sha RENAME TO idx_l1_commit_tracking_sha",
                    "ALTER TABLE l1_commit_records RENAME CONSTRAINT fk_commit_tracking TO fk_l1_commit_tracking",
                ];

                for sql in statements {
                    manager
                        .get_connection()
                        .execute(Statement::from_string(backend, sql.to_string()))
                        .await?;
                }
            }
            DatabaseBackend::MySql => {
                // MySQL: RENAME TABLE
                let statements = vec![
                    "RENAME TABLE commit_records TO l1_commit_records",
                    // MySQL 会自动重命名相关的索引和约束
                ];

                for sql in statements {
                    manager
                        .get_connection()
                        .execute(Statement::from_string(backend, sql.to_string()))
                        .await?;
                }
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();

        match backend {
            DatabaseBackend::Sqlite => {
                let statements = vec!["ALTER TABLE l1_commit_records RENAME TO commit_records"];

                for sql in statements {
                    manager
                        .get_connection()
                        .execute(Statement::from_string(backend, sql.to_string()))
                        .await?;
                }
            }
            DatabaseBackend::Postgres => {
                let statements = vec![
                    "ALTER TABLE l1_commit_records RENAME CONSTRAINT fk_l1_commit_tracking TO fk_commit_tracking",
                    "ALTER INDEX idx_l1_commit_tracking_sha RENAME TO idx_commit_tracking_sha",
                    "ALTER INDEX idx_l1_commit_type RENAME TO idx_commit_type",
                    "ALTER INDEX idx_l1_commit_status RENAME TO idx_commit_status",
                    "ALTER INDEX idx_l1_commit_sha RENAME TO idx_commit_sha",
                    "ALTER INDEX idx_l1_commit_tracking RENAME TO idx_commit_tracking",
                    "ALTER TABLE l1_commit_records RENAME TO commit_records",
                ];

                for sql in statements {
                    manager
                        .get_connection()
                        .execute(Statement::from_string(backend, sql.to_string()))
                        .await?;
                }
            }
            DatabaseBackend::MySql => {
                let statements = vec!["RENAME TABLE l1_commit_records TO commit_records"];

                for sql in statements {
                    manager
                        .get_connection()
                        .execute(Statement::from_string(backend, sql.to_string()))
                        .await?;
                }
            }
        }

        Ok(())
    }
}
