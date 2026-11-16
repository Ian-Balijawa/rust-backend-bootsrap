use crate::model::{session, user};
use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
};
use std::collections::HashSet;
use uuid::Uuid;

pub const SESSION_HOURS: i64 = 24;

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub id: i32,
    pub username: String,
    pub email: Option<String>,
    pub is_active: bool,
    pub created_at: Option<chrono::DateTime<Utc>>,
    pub updated_at: Option<chrono::DateTime<Utc>>,
    pub last_login_at: Option<chrono::DateTime<Utc>>,
    pub last_login_ip: Option<String>,
    pub permissions: HashSet<String>,
    pub session_token: String,
}

impl AuthenticatedUser {
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(permission)
    }
}

pub async fn create_session(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<(String, chrono::DateTime<Utc>), DbErr> {
    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::hours(SESSION_HOURS);
    session::ActiveModel {
        token: Set(token.clone()),
        user_id: Set(user_id),
        expires_at: Set(expires_at),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok((token, expires_at))
}

pub async fn authenticate(
    db: &DatabaseConnection,
    token: &str,
) -> Result<Option<AuthenticatedUser>, DbErr> {
    let session = session::Entity::find_by_id(token)
        .filter(session::Column::ExpiresAt.gt(Utc::now()))
        .one(db)
        .await?;
    let Some(session) = session else {
        return Ok(None);
    };
    let Some(user) = user::Entity::find_by_id(session.user_id).one(db).await? else {
        return Ok(None);
    };
    if !user.is_active || session.revoked_at.is_some() {
        return Ok(None);
    }

    let permissions = crate::service::rbac::permissions_for_user(db, user.id)
        .await?
        .into_iter()
        .collect();

    Ok(Some(AuthenticatedUser {
        id: user.id,
        username: user.username,
        email: user.email,
        is_active: user.is_active,
        created_at: user.created_at,
        updated_at: user.updated_at,
        last_login_at: user.last_login_at,
        last_login_ip: user.last_login_ip,
        permissions,
        session_token: token.to_string(),
    }))
}

pub async fn delete_session(db: &DatabaseConnection, token: &str) -> Result<(), DbErr> {
    session::Entity::delete_by_id(token).exec(db).await?;
    Ok(())
}
