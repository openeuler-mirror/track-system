use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EcosystemBindings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(EcosystemBindings::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(EcosystemBindings::TargetId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemBindings::BindType)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(EcosystemBindings::BindId).integer().null())
                    .col(
                        ColumnDef::new(EcosystemBindings::RelationRole)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EcosystemBindings::IsPrimary)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(EcosystemBindings::Metadata).json().null())
                    .col(
                        ColumnDef::new(EcosystemBindings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(EcosystemBindings::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_ecosystem_bindings_target")
                            .from(EcosystemBindings::Table, EcosystemBindings::TargetId)
                            .to(EcosystemTargets::Table, EcosystemTargets::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .name("idx_ecosystem_bindings_target")
                            .col(EcosystemBindings::TargetId),
                    )
                    .index(
                        Index::create()
                            .name("idx_ecosystem_bindings_bind")
                            .col(EcosystemBindings::BindType)
                            .col(EcosystemBindings::BindId),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(EcosystemBindings::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum EcosystemBindings {
    Table,
    Id,
    TargetId,
    BindType,
    BindId,
    RelationRole,
    IsPrimary,
    Metadata,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum EcosystemTargets {
    Table,
    Id,
}
