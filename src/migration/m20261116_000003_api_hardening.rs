use crate::model::password_reset;
use sea_orm::Schema;
use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        if manager.get_database_backend() == DbBackend::Postgres {
            db.execute_unprepared(
                "ALTER TABLE sessions ADD COLUMN IF NOT EXISTS family_id text, ADD COLUMN IF NOT EXISTS used_at timestamptz",
            )
            .await?;
        }
        // Older access sessions cannot be safely converted to refresh families.
        db.execute_unprepared("DELETE FROM sessions WHERE family_id IS NULL")
            .await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS sessions_family_idx ON sessions (family_id)",
        )
        .await?;
        db.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS users_email_lower_unique ON users (lower(email)) WHERE email IS NOT NULL",
        )
        .await?;
        if manager.get_database_backend() == DbBackend::Postgres {
            db.execute_unprepared(
                "ALTER TABLE tasks ADD COLUMN IF NOT EXISTS created_at timestamptz NOT NULL DEFAULT now(), ADD COLUMN IF NOT EXISTS updated_at timestamptz NOT NULL DEFAULT now(), ADD COLUMN IF NOT EXISTS deleted_at timestamptz",
            )
            .await?;
        }
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS tasks_owner_active_idx ON tasks (user_id, schedule_time, id) WHERE deleted_at IS NULL",
        )
        .await?;
        if manager.get_database_backend() == DbBackend::Postgres {
            db.execute_unprepared(
                "ALTER TABLE files ADD COLUMN IF NOT EXISTS updated_at timestamptz NOT NULL DEFAULT now(), ADD COLUMN IF NOT EXISTS deleted_at timestamptz",
            )
            .await?;
        }
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS files_owner_active_idx ON files (owner_id, created_at, id) WHERE deleted_at IS NULL",
        )
        .await?;
        let schema = Schema::new(manager.get_database_backend());
        let mut reset_table = schema.create_table_from_entity(password_reset::Entity);
        reset_table.if_not_exists();
        manager.create_table(reset_table).await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS password_reset_user_idx ON password_reset_tokens (user_id, expires_at)",
        )
        .await?;
        Ok(())
    }
}
