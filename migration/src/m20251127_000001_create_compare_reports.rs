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
        println!("Running m20251127_000001_create_compare_reports migration");
        let backend = manager.get_database_backend();

        if backend == DatabaseBackend::Sqlite {
            // SQLite 使用 TEXT 存储 JSON
            let create_sql = r#"
CREATE TABLE IF NOT EXISTS compare_reports (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tracking_id INTEGER NOT NULL,
    generated_at TEXT NOT NULL,
    l2_vs_l1_diff TEXT,
    l1_vs_l0_diff TEXT,
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

            let index_sql = "CREATE INDEX IF NOT EXISTS idx_compare_reports_tracking_id ON compare_reports(tracking_id)";
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
                        .table(CompareReports::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(CompareReports::Id)
                                .integer()
                                .not_null()
                                .auto_increment()
                                .primary_key(),
                        )
                        .col(
                            ColumnDef::new(CompareReports::TrackingId)
                                .integer()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(CompareReports::GeneratedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(CompareReports::L2VsL1Diff)
                                .json_binary()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(CompareReports::L1VsL0Diff)
                                .json_binary()
                                .null(),
                        )
                        .col(ColumnDef::new(CompareReports::Status).string().not_null())
                        .col(ColumnDef::new(CompareReports::FailureReason).text().null())
                        .col(
                            ColumnDef::new(CompareReports::CreatedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(CompareReports::UpdatedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .foreign_key(
                            ForeignKey::create()
                                .name("fk_compare_reports_tracking")
                                .from(CompareReports::Table, CompareReports::TrackingId)
                                .to(Tracking::Table, Tracking::Id)
                                .on_delete(ForeignKeyAction::Cascade),
                        )
                        .index(
                            Index::create()
                                .name("idx_compare_reports_tracking_id")
                                .col(CompareReports::TrackingId),
                        )
                        .to_owned(),
                )
                .await
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CompareReports::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum CompareReports {
    Table,
    Id,
    TrackingId,
    GeneratedAt,
    L2VsL1Diff,
    L1VsL0Diff,
    Status,
    FailureReason,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Tracking {
    Table,
    Id,
}
