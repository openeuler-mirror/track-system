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
        println!("Running m20251014_122500_add_app_release_layers migration");
        let backend = manager.get_database_backend();
        let spec_sql = match backend {
            DatabaseBackend::Sqlite => {
                "ALTER TABLE commit_records ADD COLUMN spec_changed INTEGER NOT NULL DEFAULT 0"
            }
            DatabaseBackend::Postgres | DatabaseBackend::MySql => {
                "ALTER TABLE commit_records ADD COLUMN spec_changed BOOLEAN NOT NULL DEFAULT false"
            }
        };

        let statements = [
            "ALTER TABLE commit_records ADD COLUMN primary_change_type TEXT",
            "ALTER TABLE commit_records ADD COLUMN cve_list BLOB",
            spec_sql,
            "ALTER TABLE commit_records ADD COLUMN patch_stats BLOB",
            "ALTER TABLE commit_records ADD COLUMN classification_status TEXT NOT NULL DEFAULT 'pending'",
            "ALTER TABLE commit_records ADD COLUMN classification_notes TEXT",
        ];

        for sql in statements {
            println!("Executing SQL: {}", sql);
            manager
                .get_connection()
                .execute(Statement::from_string(backend, sql.to_string()))
                .await
                .map_err(|err| DbErr::Custom(format!("Failed to execute `{}`: {}", sql, err)))?;
        }

        create_issues_table(manager)
            .await
            .map_err(|err| DbErr::Custom(format!("create_issues_table failed: {}", err)))?;
        create_issue_events_table(manager)
            .await
            .map_err(|err| DbErr::Custom(format!("create_issue_events_table failed: {}", err)))?;
        create_tracking_reports_table(manager)
            .await
            .map_err(|err| {
                DbErr::Custom(format!("create_tracking_reports_table failed: {}", err))
            })?;
        create_sync_jobs_table(manager)
            .await
            .map_err(|err| DbErr::Custom(format!("create_sync_jobs_table failed: {}", err)))?;
        create_l0_commits_table(manager)
            .await
            .map_err(|err| DbErr::Custom(format!("create_l0_commits_table failed: {}", err)))?;
        create_backport_candidates_table(manager)
            .await
            .map_err(|err| {
                DbErr::Custom(format!("create_backport_candidates_table failed: {}", err))
            })?;
        create_l2_snapshots_table(manager)
            .await
            .map_err(|err| DbErr::Custom(format!("create_l2_snapshots_table failed: {}", err)))?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        if backend != DatabaseBackend::Sqlite {
            for column in [
                "primary_change_type",
                "cve_list",
                "spec_changed",
                "patch_stats",
                "classification_status",
                "classification_notes",
            ] {
                let sql = format!("ALTER TABLE commit_records DROP COLUMN {}", column);
                manager
                    .get_connection()
                    .execute(Statement::from_string(backend, sql.clone()))
                    .await
                    .map_err(|err| {
                        DbErr::Custom(format!("Failed to execute `{}`: {}", sql, err))
                    })?;
            }
        }

        manager
