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
    FOREIGN KEY(tracking_id) REFERENCES tracking(id) ON DELETE CASCADE
)
"#;
        manager
            .get_connection()
            .execute(Statement::from_string(backend, create_sql.to_string()))
            .await?;

        let index_sql = "CREATE INDEX IF NOT EXISTS idx_tracking_reports_tracking_id ON tracking_reports(tracking_id)";
        manager
            .get_connection()
            .execute(Statement::from_string(backend, index_sql.to_string()))
            .await?;

        Ok(())
    } else {
        // PostgreSQL/MySQL 使用 json_binary
        manager
            .create_table(
                Table::create()
                    .table(TrackingReports::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TrackingReports::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TrackingReports::TrackingId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TrackingReports::GeneratedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TrackingReports::DiffSummary)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TrackingReports::RepresentativeChanges)
                            .json_binary()
                            .null(),
                    )
                    .col(ColumnDef::new(TrackingReports::Source).string().not_null())
                    .col(ColumnDef::new(TrackingReports::Status).string().not_null())
                    .col(ColumnDef::new(TrackingReports::FailureReason).text().null())
                    .col(
                        ColumnDef::new(TrackingReports::CreatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TrackingReports::UpdatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tracking_reports_tracking")
                            .from(TrackingReports::Table, TrackingReports::TrackingId)
                            .to(Tracking::Table, Tracking::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .name("idx_tracking_reports_tracking_id")
                            .col(TrackingReports::TrackingId),
                    )
                    .to_owned(),
            )
            .await
    }
}

async fn create_sync_jobs_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();

    if backend == DatabaseBackend::Sqlite {
        let create_sql = r#"
CREATE TABLE IF NOT EXISTS sync_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tracking_id INTEGER NOT NULL,
    job_kind TEXT NOT NULL,
    scheduled_at TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    status TEXT NOT NULL,
    error TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(tracking_id) REFERENCES tracking(id) ON DELETE CASCADE
)
"#;
        manager
            .get_connection()
            .execute(Statement::from_string(backend, create_sql.to_string()))
            .await?;

        let index_sql =
            "CREATE INDEX IF NOT EXISTS idx_sync_jobs_status ON sync_jobs(status, priority)";
        manager
            .get_connection()
            .execute(Statement::from_string(backend, index_sql.to_string()))
            .await?;

        Ok(())
    } else {
        manager
            .create_table(
                Table::create()
                    .table(SyncJobs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SyncJobs::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SyncJobs::TrackingId).integer().not_null())
                    .col(ColumnDef::new(SyncJobs::JobKind).string().not_null())
                    .col(ColumnDef::new(SyncJobs::ScheduledAt).timestamp().not_null())
                    .col(ColumnDef::new(SyncJobs::StartedAt).timestamp().null())
                    .col(ColumnDef::new(SyncJobs::FinishedAt).timestamp().null())
                    .col(ColumnDef::new(SyncJobs::Status).string().not_null())
                    .col(ColumnDef::new(SyncJobs::Error).text().null())
                    .col(
                        ColumnDef::new(SyncJobs::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(SyncJobs::Priority)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(SyncJobs::CreatedAt).timestamp().not_null())
                    .col(ColumnDef::new(SyncJobs::UpdatedAt).timestamp().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sync_jobs_tracking")
                            .from(SyncJobs::Table, SyncJobs::TrackingId)
                            .to(Tracking::Table, Tracking::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .name("idx_sync_jobs_status")
                            .col(SyncJobs::Status)
                            .col(SyncJobs::Priority),
                    )
                    .to_owned(),
            )
            .await
    }
}

async fn create_l0_commits_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();

    if backend == DatabaseBackend::Sqlite {
        let create_sql = r#"
CREATE TABLE IF NOT EXISTS l0_commits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    package_id INTEGER NOT NULL,
    repo TEXT NOT NULL,
    commit_sha TEXT NOT NULL,
    summary TEXT NOT NULL,
    authored_at TEXT NOT NULL,
    metadata TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE
)
"#;
        manager
            .get_connection()
            .execute(Statement::from_string(backend, create_sql.to_string()))
            .await?;

        let index_sql = "CREATE INDEX IF NOT EXISTS idx_l0_commits_package_sha ON l0_commits(package_id, commit_sha)";
        manager
            .get_connection()
            .execute(Statement::from_string(backend, index_sql.to_string()))
            .await?;

        Ok(())
    } else {
        manager
            .create_table(
                Table::create()
                    .table(L0Commits::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(L0Commits::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(L0Commits::PackageId).integer().not_null())
                    .col(ColumnDef::new(L0Commits::Repo).string().not_null())
                    .col(ColumnDef::new(L0Commits::CommitSha).string().not_null())
                    .col(ColumnDef::new(L0Commits::Summary).text().not_null())
                    .col(ColumnDef::new(L0Commits::AuthoredAt).timestamp().not_null())
                    .col(ColumnDef::new(L0Commits::Metadata).json_binary().null())
                    .col(ColumnDef::new(L0Commits::CreatedAt).timestamp().not_null())
                    .col(ColumnDef::new(L0Commits::UpdatedAt).timestamp().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_l0_commits_package")
                            .from(L0Commits::Table, L0Commits::PackageId)
                            .to(Packages::Table, Packages::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .name("idx_l0_commits_package_sha")
                            .col(L0Commits::PackageId)
                            .col(L0Commits::CommitSha),
                    )
                    .to_owned(),
            )
            .await
    }
}

async fn create_backport_candidates_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();

    if backend == DatabaseBackend::Sqlite {
        let create_sql = r#"
CREATE TABLE IF NOT EXISTS backport_candidates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    package_id INTEGER NOT NULL,
    l0_commit_id INTEGER NOT NULL,
    target_distro_id INTEGER NOT NULL,
    spec_base_version TEXT NOT NULL,
    recommendation TEXT NOT NULL,
    status TEXT NOT NULL,
    patch_artifact TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE,
    FOREIGN KEY(l0_commit_id) REFERENCES l0_commits(id) ON DELETE CASCADE,
    FOREIGN KEY(target_distro_id) REFERENCES distros(id) ON DELETE CASCADE
)
"#;
        manager
            .get_connection()
            .execute(Statement::from_string(backend, create_sql.to_string()))
            .await?;

        let index_sql = "CREATE INDEX IF NOT EXISTS idx_backport_candidates_pkg_status ON backport_candidates(package_id, status)";
        manager
            .get_connection()
            .execute(Statement::from_string(backend, index_sql.to_string()))
            .await?;

        Ok(())
    } else {
        manager
            .create_table(
                Table::create()
                    .table(BackportCandidates::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BackportCandidates::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BackportCandidates::PackageId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BackportCandidates::L0CommitId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BackportCandidates::TargetDistroId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BackportCandidates::SpecBaseVersion)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BackportCandidates::Recommendation)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BackportCandidates::Status)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BackportCandidates::PatchArtifact)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(BackportCandidates::CreatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BackportCandidates::UpdatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_backport_candidates_package")
                            .from(BackportCandidates::Table, BackportCandidates::PackageId)
                            .to(Packages::Table, Packages::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_backport_candidates_l0_commit")
                            .from(BackportCandidates::Table, BackportCandidates::L0CommitId)
                            .to(L0Commits::Table, L0Commits::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_backport_candidates_target_distro")
                            .from(
                                BackportCandidates::Table,
                                BackportCandidates::TargetDistroId,
                            )
                            .to(Distros::Table, Distros::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .name("idx_backport_candidates_pkg_status")
                            .col(BackportCandidates::PackageId)
                            .col(BackportCandidates::Status),
                    )
                    .to_owned(),
            )
            .await
    }
}

async fn create_l2_snapshots_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();

    if backend == DatabaseBackend::Sqlite {
        let create_sql = r#"
CREATE TABLE IF NOT EXISTS l2_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tracking_id INTEGER NOT NULL,
    snapshot_type TEXT NOT NULL,
    checksum TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(tracking_id) REFERENCES tracking(id) ON DELETE CASCADE
)
"#;
        manager
            .get_connection()
            .execute(Statement::from_string(backend, create_sql.to_string()))
            .await?;

        let index_sql =
            "CREATE INDEX IF NOT EXISTS idx_l2_snapshots_tracking ON l2_snapshots(tracking_id)";
        manager
            .get_connection()
            .execute(Statement::from_string(backend, index_sql.to_string()))
            .await?;

        Ok(())
    } else {
        manager
            .create_table(
                Table::create()
                    .table(L2Snapshots::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(L2Snapshots::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
