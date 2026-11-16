//! PostgreSQL persistence for file metadata. Bytes are managed by the file service.

use crate::model::file;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DbErr, EntityTrait, Set};

pub async fn insert(
    db: &DatabaseConnection,
    id: String,
    owner_id: i32,
    filename: &str,
    content_type: &str,
    size_bytes: i64,
) -> Result<file::Model, DbErr> {
    file::ActiveModel {
        id: Set(id),
        owner_id: Set(owner_id),
        filename: Set(filename.to_string()),
        content_type: Set(content_type.to_string()),
        size_bytes: Set(size_bytes),
        created_at: Set(Utc::now()),
    }
    .insert(db)
    .await
}

pub async fn find(db: &DatabaseConnection, id: &str) -> Result<Option<file::Model>, DbErr> {
    file::Entity::find_by_id(id).one(db).await
}

pub async fn delete(db: &DatabaseConnection, id: &str) -> Result<(), DbErr> {
    file::Entity::delete_by_id(id).exec(db).await?;
    Ok(())
}
