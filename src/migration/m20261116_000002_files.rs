use crate::model::file;
use sea_orm::Schema;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(manager.get_database_backend());
        let mut table = schema.create_table_from_entity(file::Entity);
        table.if_not_exists();
        manager.create_table(table).await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_files_owner_id")
                    .table(file::Entity)
                    .col(file::Column::OwnerId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }
}
