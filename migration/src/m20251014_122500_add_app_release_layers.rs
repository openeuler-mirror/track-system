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
            .drop_table(Table::drop().table(L2Snapshots::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(BackportCandidates::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(L0Commits::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SyncJobs::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(TrackingReports::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(IssueEvents::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Issues::Table).to_owned())
            .await?;

        Ok(())
    }
}

async fn create_issues_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();
    if backend == DatabaseBackend::Sqlite {
        let create_sql = r#"
CREATE TABLE IF NOT EXISTS issues (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tracking_id INTEGER NOT NULL,
    issue_number TEXT NOT NULL,
    title TEXT NOT NULL,
    state TEXT NOT NULL,
    author TEXT NOT NULL,
    api_url TEXT NOT NULL,
    labels TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    closed_at TEXT,
    raw_payload TEXT,
    FOREIGN KEY(tracking_id) REFERENCES tracking(id) ON DELETE CASCADE
);
"#;
        manager
            .get_connection()
            .execute(Statement::from_string(backend, create_sql.to_string()))
            .await?;

        let index_sql = "CREATE INDEX IF NOT EXISTS idx_issues_tracking_number ON issues(tracking_id, issue_number)";
        manager
            .get_connection()
            .execute(Statement::from_string(backend, index_sql.to_string()))
            .await?;

        Ok(())
    } else {
        manager
            .create_table(
                Table::create()
                    .table(Issues::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Issues::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Issues::TrackingId).integer().not_null())
                    .col(ColumnDef::new(Issues::IssueNumber).string().not_null())
                    .col(ColumnDef::new(Issues::Title).text().not_null())
                    .col(ColumnDef::new(Issues::State).string().not_null())
                    .col(ColumnDef::new(Issues::Author).string().not_null())
                    .col(ColumnDef::new(Issues::ApiUrl).string().not_null())
                    .col(ColumnDef::new(Issues::Labels).json_binary().null())
                    .col(ColumnDef::new(Issues::CreatedAt).timestamp().not_null())
                    .col(ColumnDef::new(Issues::UpdatedAt).timestamp().not_null())
                    .col(ColumnDef::new(Issues::ClosedAt).timestamp().null())
                    .col(ColumnDef::new(Issues::RawPayload).json_binary().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_issues_tracking")
                            .from(Issues::Table, Issues::TrackingId)
                            .to(Tracking::Table, Tracking::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .name("idx_issues_tracking_number")
                            .col(Issues::TrackingId)
                            .col(Issues::IssueNumber),
                    )
                    .to_owned(),
            )
            .await
    }
}

async fn create_issue_events_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(IssueEvents::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(IssueEvents::Id)
                        .integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(ColumnDef::new(IssueEvents::IssueId).integer().not_null())
                .col(ColumnDef::new(IssueEvents::EventType).string().not_null())
                .col(ColumnDef::new(IssueEvents::Actor).string().null())
                .col(ColumnDef::new(IssueEvents::EventAt).timestamp().not_null())
                .col(ColumnDef::new(IssueEvents::Payload).json_binary().null())
                .col(
                    ColumnDef::new(IssueEvents::CreatedAt)
                        .timestamp()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_issue_events_issue")
                        .from(IssueEvents::Table, IssueEvents::IssueId)
                        .to(Issues::Table, Issues::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await
}

async fn create_tracking_reports_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();

    if backend == DatabaseBackend::Sqlite {
        // SQLite 使用 TEXT 存储 JSON
        let create_sql = r#"
CREATE TABLE IF NOT EXISTS tracking_reports (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tracking_id INTEGER NOT NULL,
    generated_at TEXT NOT NULL,
    diff_summary TEXT NOT NULL,
    representative_changes TEXT,
    source TEXT NOT NULL,
    status TEXT NOT NULL,
    failure_reason TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
