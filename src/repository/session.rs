//! PostgreSQL persistence for hashed refresh tokens.

use crate::model::session;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
    sea_query::Expr,
};
use sha2::{Digest, Sha256};

fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

pub async fn insert<C: ConnectionTrait>(
    db: &C,
    token: &str,
    user_id: i32,
    expires_at: DateTime<Utc>,
    family_id: &str,
) -> Result<(), DbErr> {
    session::ActiveModel {
        token: Set(token_hash(token)),
        user_id: Set(user_id),
        expires_at: Set(expires_at),
        family_id: Set(Some(family_id.to_owned())),
        used_at: Set(None),
        revoked_at: Set(None),
        created_at: Set(Some(Utc::now())),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

pub async fn find_any<C: ConnectionTrait>(
    db: &C,
    token: &str,
) -> Result<Option<session::Model>, DbErr> {
    session::Entity::find_by_id(token_hash(token)).one(db).await
}

pub async fn find_valid<C: ConnectionTrait>(
    db: &C,
    token: &str,
) -> Result<Option<session::Model>, DbErr> {
    session::Entity::find_by_id(token_hash(token))
        .filter(session::Column::ExpiresAt.gt(Utc::now()))
        .filter(session::Column::RevokedAt.is_null())
        .filter(session::Column::UsedAt.is_null())
        .one(db)
        .await
}

pub async fn consume<C: ConnectionTrait>(db: &C, token: &str) -> Result<bool, DbErr> {
    let result = session::Entity::update_many()
        .col_expr(session::Column::UsedAt, Expr::value(Utc::now()))
        .filter(session::Column::Token.eq(token_hash(token)))
        .filter(session::Column::ExpiresAt.gt(Utc::now()))
        .filter(session::Column::RevokedAt.is_null())
        .filter(session::Column::UsedAt.is_null())
        .exec(db)
        .await?;
    Ok(result.rows_affected == 1)
}

pub async fn revoke_family<C: ConnectionTrait>(db: &C, family_id: &str) -> Result<(), DbErr> {
    session::Entity::update_many()
        .col_expr(session::Column::RevokedAt, Expr::value(Utc::now()))
        .filter(session::Column::FamilyId.eq(family_id))
        .filter(session::Column::RevokedAt.is_null())
        .exec(db)
        .await?;
    Ok(())
}

pub async fn revoke_for_user<C: ConnectionTrait>(
    db: &C,
    user_id: i32,
    token: &str,
) -> Result<(), DbErr> {
    session::Entity::update_many()
        .col_expr(session::Column::RevokedAt, Expr::value(Utc::now()))
        .filter(session::Column::Token.eq(token_hash(token)))
        .filter(session::Column::UserId.eq(user_id))
        .filter(session::Column::RevokedAt.is_null())
        .exec(db)
        .await?;
    Ok(())
}

pub async fn delete_all_for_user<C: ConnectionTrait>(db: &C, user_id: i32) -> Result<(), DbErr> {
    session::Entity::update_many()
        .col_expr(session::Column::RevokedAt, Expr::value(Utc::now()))
        .filter(session::Column::UserId.eq(user_id))
        .filter(session::Column::RevokedAt.is_null())
        .exec(db)
        .await?;
    Ok(())
}
