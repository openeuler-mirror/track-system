use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EcosystemEvidenceSnapshots::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(EcosystemEvidenceSnapshots::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(EcosystemEvidenceSnapshots::TargetId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemEvidenceSnapshots::SourceType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemEvidenceSnapshots::SourceName)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemEvidenceSnapshots::SourceUrl)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemEvidenceSnapshots::HttpStatus)
                            .integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemEvidenceSnapshots::ContentHash)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemEvidenceSnapshots::RawPayload)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemEvidenceSnapshots::NormalizedSignals)
                            .json()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemEvidenceSnapshots::CollectedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemEvidenceSnapshots::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(EcosystemEvidenceSnapshots::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_ecosystem_evidence_target")
                            .from(
                                EcosystemEvidenceSnapshots::Table,
                                EcosystemEvidenceSnapshots::TargetId,
                            )
                            .to(EcosystemTargets::Table, EcosystemTargets::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .name("idx_ecosystem_evidence_target")
                            .col(EcosystemEvidenceSnapshots::TargetId),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(EcosystemEvidenceSnapshots::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum EcosystemEvidenceSnapshots {
    Table,
    Id,
    TargetId,
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
enum EcosystemTargets {
    Table,
    Id,
}
