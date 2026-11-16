use crate::{repository, service};
use chrono::{Duration, Utc};
use sea_orm::{DatabaseConnection, DbErr};
use std::collections::HashSet;
use uuid::Uuid;

pub struct LoginResult {
    pub user_id: i32,
    pub last_login_at: Option<chrono::DateTime<Utc>>,
    pub token: String,
    pub expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("too many login attempts; try again later")]
    Limited,
    #[error("invalid username or password")]
    InvalidCredentials,
    #[error("user account is disabled")]
    Disabled,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub async fn login(
    db: &DatabaseConnection,
    redis: &redis::aio::ConnectionManager,
    username: &str,
    password: &str,
    ip: &str,
    user_agent: Option<&str>,
) -> Result<LoginResult, LoginError> {
    if service::redis::is_login_limited(redis, username, ip)
        .await
        .map_err(anyhow::Error::from)?
    {
        return Err(LoginError::Limited);
    }
    let user = repository::user::find_by_username(db, username)
        .await
        .map_err(anyhow::Error::from)?;
    let Some(user) = user else {
        service::redis::record_login_failure(redis, username, ip)
            .await
            .map_err(anyhow::Error::from)?;
        log_login(
            db,
            None,
            username,
            false,
            ip,
            user_agent,
            Some("invalid credentials"),
        )
        .await;
        return Err(LoginError::InvalidCredentials);
    };
    if !user.is_active {
        log_login(
            db,
            Some(user.id),
            username,
            false,
            ip,
            user_agent,
            Some("account disabled"),
        )
        .await;
        return Err(LoginError::Disabled);
    }
    if !service::user::verify_password(&user.password_hash, password) {
        service::redis::record_login_failure(redis, username, ip)
            .await
            .map_err(anyhow::Error::from)?;
        log_login(
            db,
            Some(user.id),
            username,
            false,
            ip,
            user_agent,
            Some("invalid credentials"),
        )
        .await;
        return Err(LoginError::InvalidCredentials);
    }

    let (user, token, expires_at) = service::user::record_login(db, user, ip).await?;
    service::redis::clear_login_failures(redis, username, ip)
        .await
        .map_err(anyhow::Error::from)?;
    log_login(
        db,
        Some(user.id),
        &user.username,
        true,
        ip,
        user_agent,
        None,
    )
    .await;
    Ok(LoginResult {
        user_id: user.id,
        last_login_at: user.last_login_at,
        token,
        expires_at,
    })
}

async fn log_login(
    db: &DatabaseConnection,
    user_id: Option<i32>,
    username: &str,
    success: bool,
    ip: &str,
    user_agent: Option<&str>,
    reason: Option<&str>,
) {
    if let Err(error) =
        repository::login_log::record(db, user_id, username, success, ip, user_agent, reason).await
    {
        tracing::warn!(%error, "failed to record login event");
    }
}

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
    repository::session::insert(db, &token, user_id, expires_at).await?;
    Ok((token, expires_at))
}

pub async fn authenticate(
    db: &DatabaseConnection,
    token: &str,
) -> Result<Option<AuthenticatedUser>, DbErr> {
    let session = repository::session::find_valid(db, token).await?;
    let Some(session) = session else {
        return Ok(None);
    };
    let Some(user) = repository::user::find_by_id(db, session.user_id).await? else {
        return Ok(None);
    };
    if !user.is_active {
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
    repository::session::delete(db, token).await
}
