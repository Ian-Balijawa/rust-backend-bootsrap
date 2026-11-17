use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let ddl = if manager.get_database_backend() == DbBackend::Postgres {
            "CREATE TABLE IF NOT EXISTS notes (
                id uuid PRIMARY KEY,
                owner_id integer NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                title text NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
                body text NOT NULL CHECK (length(body) <= 10000),
                created_at timestamptz NOT NULL DEFAULT now(),
                updated_at timestamptz NOT NULL DEFAULT now(),
                deleted_at timestamptz
            )"
        } else {
            "CREATE TABLE IF NOT EXISTS notes (
                id text PRIMARY KEY,
                owner_id integer NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                title text NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
                body text NOT NULL CHECK (length(body) <= 10000),
                created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
                deleted_at text
            )"
        };
        manager.get_connection().execute_unprepared(ddl).await?;
        manager.get_connection().execute_unprepared(
            "CREATE INDEX IF NOT EXISTS notes_owner_active_idx ON notes (owner_id, created_at, id) WHERE deleted_at IS NULL",
        ).await?;
        Ok(())
    }
}
