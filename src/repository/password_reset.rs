use crate::model::password_reset;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
    sea_query::Expr,
};

pub async fn insert<C: ConnectionTrait>(
    db: &C,
    token_hash: &str,
    user_id: i32,
    expires_at: DateTime<Utc>,
) -> Result<(), DbErr> {
    password_reset::ActiveModel {
        token_hash: Set(token_hash.to_owned()),
        user_id: Set(user_id),
        expires_at: Set(expires_at),
        used_at: Set(None),
        created_at: Set(Utc::now()),
    }
    .insert(db)
    .await?;
    Ok(())
}

pub async fn consume<C: ConnectionTrait>(db: &C, token_hash: &str) -> Result<Option<i32>, DbErr> {
    let result = password_reset::Entity::update_many()
        .col_expr(password_reset::Column::UsedAt, Expr::value(Utc::now()))
        .filter(password_reset::Column::TokenHash.eq(token_hash))
        .filter(password_reset::Column::UsedAt.is_null())
        .filter(password_reset::Column::ExpiresAt.gt(Utc::now()))
        .exec(db)
        .await?;
    if result.rows_affected == 0 {
        return Ok(None);
    }
    Ok(password_reset::Entity::find_by_id(token_hash)
        .one(db)
        .await?
        .map(|row| row.user_id))
}
