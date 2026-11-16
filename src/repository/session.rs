//! PostgreSQL persistence for authentication sessions.

use crate::model::session;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
};

pub async fn insert(
    db: &DatabaseConnection,
    token: &str,
    user_id: i32,
    expires_at: DateTime<Utc>,
) -> Result<(), DbErr> {
    session::ActiveModel {
        token: Set(token.to_string()),
        user_id: Set(user_id),
        expires_at: Set(expires_at),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

pub async fn find_valid(
    db: &DatabaseConnection,
    token: &str,
) -> Result<Option<session::Model>, DbErr> {
    session::Entity::find_by_id(token)
        .filter(session::Column::ExpiresAt.gt(Utc::now()))
        .filter(session::Column::RevokedAt.is_null())
        .one(db)
        .await
}

pub async fn delete(db: &DatabaseConnection, token: &str) -> Result<(), DbErr> {
    session::Entity::delete_by_id(token).exec(db).await?;
    Ok(())
}
