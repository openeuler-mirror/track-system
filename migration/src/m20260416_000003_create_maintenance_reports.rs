use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        let id_column = match backend {
            DatabaseBackend::Sqlite => ColumnDef::new(MaintenanceReports::Id)
                .integer()
                .not_null()
                .auto_increment()
                .primary_key()
                .to_owned(),
            _ => ColumnDef::new(MaintenanceReports::Id)
                .big_integer()
                .not_null()
                .auto_increment()
                .primary_key()
                .to_owned(),
        };

        manager
            .create_table(
                Table::create()
                    .table(MaintenanceReports::Table)
                    .if_not_exists()
                    .col(id_column)
                    .col(
                        ColumnDef::new(MaintenanceReports::PackageId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceReports::ReportType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceReports::Status)
                            .string()
                            .not_null()
                            .default("completed"),
                    )
                    .col(
                        ColumnDef::new(MaintenanceReports::OverallRisk)
                            .string()
                            .not_null()
                            .default("UNKNOWN"),
                    )
                    .col(
                        ColumnDef::new(MaintenanceReports::Confidence)
                            .string()
                            .not_null()
                            .default("LOW"),
                    )
                    .col(
                        ColumnDef::new(MaintenanceReports::Summary)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceReports::Dimensions)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceReports::EvidenceSummary)
                            .json()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceReports::ReportPayload)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceReports::GeneratedAt)
                            .custom(timestamp_type(backend))
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MaintenanceReports::CreatedAt)
                            .custom(timestamp_type(backend))
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MaintenanceReports::UpdatedAt)
                            .custom(timestamp_type(backend))
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_maintenance_reports_target")
                            .from(MaintenanceReports::Table, MaintenanceReports::PackageId)
                            .to(Packages::Table, Packages::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_maintenance_reports_target")
