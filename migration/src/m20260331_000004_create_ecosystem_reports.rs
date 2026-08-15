use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        let id_column = match backend {
            DatabaseBackend::Sqlite => ColumnDef::new(EcosystemReports::Id)
                .integer()
                .not_null()
                .auto_increment()
                .primary_key()
                .to_owned(),
            _ => ColumnDef::new(EcosystemReports::Id)
                .big_integer()
                .not_null()
                .auto_increment()
                .primary_key()
                .to_owned(),
        };
        manager
            .create_table(
                Table::create()
                    .table(EcosystemReports::Table)
                    .if_not_exists()
                    .col(id_column)
                    .col(
                        ColumnDef::new(EcosystemReports::TargetId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemReports::ReportType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemReports::Status)
                            .string()
                            .not_null()
                            .default("completed"),
                    )
                    .col(
                        ColumnDef::new(EcosystemReports::OverallRisk)
                            .string()
                            .not_null()
                            .default("UNKNOWN"),
                    )
                    .col(
                        ColumnDef::new(EcosystemReports::Confidence)
                            .string()
                            .not_null()
                            .default("LOW"),
                    )
                    .col(ColumnDef::new(EcosystemReports::Summary).text().not_null())
                    .col(
                        ColumnDef::new(EcosystemReports::Dimensions)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemReports::EvidenceSummary)
                            .json()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemReports::ReportPayload)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemReports::GeneratedAt)
                            .custom(timestamp_type(backend))
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemReports::CreatedAt)
                            .custom(timestamp_type(backend))
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(EcosystemReports::UpdatedAt)
                            .custom(timestamp_type(backend))
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_ecosystem_reports_target")
                            .from(EcosystemReports::Table, EcosystemReports::TargetId)
                            .to(EcosystemTargets::Table, EcosystemTargets::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_ecosystem_reports_target")
                    .table(EcosystemReports::Table)
                    .col(EcosystemReports::TargetId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(EcosystemReports::Table).to_owned())
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
enum EcosystemReports {
    Table,
    Id,
    TargetId,
    ReportType,
    Status,
    OverallRisk,
    Confidence,
    Summary,
    Dimensions,
    EvidenceSummary,
    ReportPayload,
    GeneratedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum EcosystemTargets {
    Table,
    Id,
}
