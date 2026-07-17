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
