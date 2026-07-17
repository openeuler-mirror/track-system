use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        let id_column = match backend {
            DatabaseBackend::Sqlite => ColumnDef::new(MaintenanceEvidenceSnapshots::Id)
                .integer()
                .not_null()
                .auto_increment()
                .primary_key()
                .to_owned(),
            _ => ColumnDef::new(MaintenanceEvidenceSnapshots::Id)
                .big_integer()
                .not_null()
                .auto_increment()
                .primary_key()
                .to_owned(),
        };

        manager
            .create_table(
                Table::create()
                    .table(MaintenanceEvidenceSnapshots::Table)
                    .if_not_exists()
                    .col(id_column)
                    .col(
                        ColumnDef::new(MaintenanceEvidenceSnapshots::PackageId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceEvidenceSnapshots::SourceType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceEvidenceSnapshots::SourceName)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceEvidenceSnapshots::SourceUrl)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceEvidenceSnapshots::HttpStatus)
                            .integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceEvidenceSnapshots::ContentHash)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceEvidenceSnapshots::RawPayload)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceEvidenceSnapshots::NormalizedSignals)
                            .json()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceEvidenceSnapshots::CollectedAt)
                            .custom(timestamp_type(backend))
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceEvidenceSnapshots::CreatedAt)
                            .custom(timestamp_type(backend))
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MaintenanceEvidenceSnapshots::UpdatedAt)
                            .custom(timestamp_type(backend))
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_maintenance_evidence_target")
                            .from(
                                MaintenanceEvidenceSnapshots::Table,
                                MaintenanceEvidenceSnapshots::PackageId,
                            )
                            .to(Packages::Table, Packages::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_maintenance_evidence_target")
                    .table(MaintenanceEvidenceSnapshots::Table)
                    .col(MaintenanceEvidenceSnapshots::PackageId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(MaintenanceEvidenceSnapshots::Table)
                    .to_owned(),
            )
            .await
    }
}

fn timestamp_type(backend: DatabaseBackend) -> Alias {
    match backend {
        DatabaseBackend::Postgres => Alias::new("timestamp with time zone"),
        _ => Alias::new("timestamp"),
    }
}

#[derive(DeriveIden)]
enum MaintenanceEvidenceSnapshots {
    Table,
    Id,
    PackageId,
    SourceType,
    SourceName,
    SourceUrl,
    HttpStatus,
    ContentHash,
    RawPayload,
    NormalizedSignals,
    CollectedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Packages {
    Table,
    Id,
}
