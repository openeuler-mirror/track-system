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
        let is_binary_sql = match backend {
            DatabaseBackend::Sqlite => {
                "ALTER TABLE commit_files ADD COLUMN is_binary INTEGER NOT NULL DEFAULT 0"
            }
            DatabaseBackend::Postgres | DatabaseBackend::MySql => {
                "ALTER TABLE commit_files ADD COLUMN is_binary BOOLEAN NOT NULL DEFAULT false"
            }
        };

        let statements = [
            "ALTER TABLE commit_files ADD COLUMN patch_content TEXT",
            "ALTER TABLE commit_files ADD COLUMN patch_format TEXT",
            is_binary_sql,
            "ALTER TABLE commit_files ADD COLUMN old_mode TEXT",
            "ALTER TABLE commit_files ADD COLUMN new_mode TEXT",
            "ALTER TABLE commit_files ADD COLUMN updated_at TEXT",
        ];

        for sql in statements {
            manager
                .get_connection()
                .execute(Statement::from_string(backend, sql.to_string()))
                .await
                .map_err(|err| DbErr::Custom(format!("Failed to execute `{}`: {}", sql, err)))?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        if backend != DatabaseBackend::Sqlite {
            for column in [
                "patch_content",
                "patch_format",
                "is_binary",
                "old_mode",
                "new_mode",
                "updated_at",
            ] {
                let sql = format!("ALTER TABLE commit_files DROP COLUMN {}", column);
                manager
                    .get_connection()
                    .execute(Statement::from_string(backend, sql))
                    .await
                    .map_err(|err| {
                        DbErr::Custom(format!("Failed to execute `{}`: {}", column, err))
                    })?;
            }
        }

        Ok(())
    }
}
