//! PostgreSQL persistence for users.

use crate::model::user;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
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
        .filter(user::Column::Email.eq(email))
        .one(db)
        .await
}

pub async fn find_by_id(db: &DatabaseConnection, id: i32) -> Result<Option<user::Model>, DbErr> {
    user::Entity::find_by_id(id).one(db).await
}

pub async fn insert(
    db: &DatabaseConnection,
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
