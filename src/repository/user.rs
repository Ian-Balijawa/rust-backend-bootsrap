//! PostgreSQL persistence for users.

use crate::model::user;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, Set,
    sea_query::{Expr, ExprTrait, Func},
};

pub async fn find_by_username(
    db: &DatabaseConnection,
    username: &str,
) -> Result<Option<user::Model>, DbErr> {
    user::Entity::find()
        .filter(user::Column::Username.eq(username))
        .one(db)
        .await
}

pub async fn find_by_email(
    db: &DatabaseConnection,
    email: &str,
) -> Result<Option<user::Model>, DbErr> {
    user::Entity::find()
        .filter(Func::lower(Expr::col(user::Column::Email)).eq(email.trim().to_lowercase()))
        .one(db)
        .await
}

pub async fn update_password<C>(db: &C, id: i32, password_hash: String) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let result = user::Entity::update_many()
        .col_expr(user::Column::PasswordHash, Expr::value(password_hash))
        .col_expr(user::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(user::Column::Id.eq(id))
        .exec(db)
        .await?;
    if result.rows_affected == 0 {
        Err(DbErr::RecordNotFound("user not found".to_string()))
    } else {
        Ok(())
    }
}

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i32) -> Result<Option<user::Model>, DbErr> {
    user::Entity::find_by_id(id).one(db).await
}

pub async fn insert<C: ConnectionTrait>(
    db: &C,
    username: &str,
    password_hash: String,
    email: Option<&str>,
) -> Result<user::Model, DbErr> {
    let now = Utc::now();
    user::ActiveModel {
        username: Set(username.to_string()),
        password_hash: Set(password_hash),
        email: Set(email.map(str::to_string)),
        is_active: Set(true),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
        last_login_at: Set(None),
        last_login_ip: Set(None),
        ..Default::default()
    }
    .insert(db)
    .await
}

pub async fn record_login(
    db: &DatabaseConnection,
    user: user::Model,
    ip: &str,
) -> Result<user::Model, DbErr> {
    let mut active: user::ActiveModel = user.into();
    let now: DateTime<Utc> = Utc::now();
    active.last_login_at = Set(Some(now));
    active.last_login_ip = Set(Some(ip.to_string()));
    active.updated_at = Set(Some(now));
    active.update(db).await
}
