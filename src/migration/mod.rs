mod m20260902_000001_initial_schema;
mod m20261116_000002_files;
mod m20261116_000003_api_hardening;
mod m20261117_000004_notes;

use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260902_000001_initial_schema::Migration),
            Box::new(m20261116_000002_files::Migration),
            Box::new(m20261116_000003_api_hardening::Migration),
            Box::new(m20261117_000004_notes::Migration),
        ]
    }
}
