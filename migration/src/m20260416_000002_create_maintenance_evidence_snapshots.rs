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
