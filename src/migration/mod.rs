mod m20260902_000001_initial_schema;
mod m20261116_000002_files;

use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260902_000001_initial_schema::Migration),
            Box::new(m20261116_000002_files::Migration),
        ]
    }
}
