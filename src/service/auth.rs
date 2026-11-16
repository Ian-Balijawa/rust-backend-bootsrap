use crate::config::Config;
use crate::{repository, service};
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sea_orm::{DatabaseConnection, DbErr, TransactionTrait};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

pub struct LoginResult {
    pub user_id: i32,
    pub last_login_at: Option<chrono::DateTime<Utc>>,
    pub token: String,
    pub expires_at: chrono::DateTime<Utc>,
    pub refresh_token: String,
    pub refresh_expires_at: chrono::DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
struct AccessClaims {
    sub: String,
    role: String,
    exp: i64,
    iss: String,
    aud: String,
}

const ACCESS_MINUTES: i64 = 15;
const REFRESH_DAYS: i64 = 14;

fn new_refresh_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn role_for_permissions(permissions: &[String]) -> &'static str {
    if permissions.iter().any(|permission| permission == "user:manage") {
        "admin"
    } else {
        "user"
    }
}

pub async fn issue_access_token(
    db: &DatabaseConnection,
    config: &Config,
    user_id: i32,
) -> anyhow::Result<(String, chrono::DateTime<Utc>)> {
    let permissions = service::rbac::permissions_for_user(db, user_id).await?;
    let expires_at = Utc::now() + Duration::minutes(ACCESS_MINUTES);
    let claims = AccessClaims {
        sub: user_id.to_string(),
        role: role_for_permissions(&permissions).to_string(),
        exp: expires_at.timestamp(),
        iss: config.jwt_issuer.clone(),
        aud: config.jwt_audience.clone(),
    };
    let key = EncodingKey::from_secret(config.jwt_secret.as_bytes())?;
    let token = encode(&Header::new(Algorithm::HS256), &claims, &key)?;
    Ok((token, expires_at))
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("too many login attempts; try again later")]
    Limited,
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub async fn login(
    db: &DatabaseConnection,
    redis: &redis::aio::ConnectionManager,
    config: &Config,
    email: &str,
    password: &str,
    ip: &str,
    user_agent: Option<&str>,
) -> Result<LoginResult, LoginError> {
    let email = email.trim().to_lowercase();
    if service::redis::is_login_limited(redis, &email, ip)
        .await
        .map_err(anyhow::Error::from)?
    {
        return Err(LoginError::Limited);
    }
    let user = repository::user::find_by_email(db, &email)
        .await
        .map_err(anyhow::Error::from)?;
    let Some(user) = user else {
        let _ = service::user::hash_password(password).await;
        service::redis::record_login_failure(redis, &email, ip)
            .await
            .map_err(anyhow::Error::from)?;
        log_login(
            db,
            None,
            &email,
            false,
            ip,
            user_agent,
            Some("invalid credentials"),
        )
        .await;
        return Err(LoginError::InvalidCredentials);
    };
    if !user.is_active {
        let _ = service::user::verify_password(&user.password_hash, password).await;
        service::redis::record_login_failure(redis, &email, ip)
            .await
            .map_err(anyhow::Error::from)?;
        log_login(
            db,
            Some(user.id),
            &email,
            false,
            ip,
            user_agent,
            Some("account disabled"),
        )
        .await;
        return Err(LoginError::InvalidCredentials);
    }
    if !service::user::verify_password(&user.password_hash, password).await {
        service::redis::record_login_failure(redis, &email, ip)
            .await
            .map_err(anyhow::Error::from)?;
        log_login(
            db,
            Some(user.id),
            &email,
            false,
            ip,
            user_agent,
            Some("invalid credentials"),
        )
        .await;
        return Err(LoginError::InvalidCredentials);
    }

    if user.password_hash.starts_with("$2") {
        let upgraded_hash = service::user::hash_password(password).await?;
        repository::user::update_password(db, user.id, upgraded_hash)
            .await
            .map_err(anyhow::Error::from)?;
    }

    let user = service::user::record_login(db, user, ip).await?;
    let (refresh_token, refresh_expires_at) = create_session(db, user.id)
        .await
        .map_err(anyhow::Error::from)?;
    let (token, expires_at) = issue_access_token(db, config, user.id).await?;
    service::redis::clear_login_failures(redis, &email, ip)
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
        refresh_token,
        refresh_expires_at,
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

#[derive(Clone)]
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
    pub access_token: String,
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
    let token = new_refresh_token();
    let expires_at = Utc::now() + Duration::days(REFRESH_DAYS);
    let family_id = Uuid::now_v7().to_string();
    repository::session::insert(db, &token, user_id, expires_at, &family_id).await?;
    Ok((token, expires_at))
}

pub async fn authenticate_access(
    db: &DatabaseConnection,
    config: &Config,
    token: &str,
) -> Result<Option<AuthenticatedUser>, DbErr> {
    let Ok(key) = DecodingKey::from_secret(config.jwt_secret.as_bytes()) else {
        return Ok(None);
    };
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[&config.jwt_issuer]);
    validation.set_audience(&[&config.jwt_audience]);
    validation.required_spec_claims.extend(["sub", "iss", "aud"].map(str::to_string));
    validation.leeway = 0;
    let Ok(decoded) = decode::<AccessClaims>(token, &key, &validation) else {
        return Ok(None);
    };
    let Ok(user_id) = decoded.claims.sub.parse::<i32>() else {
        return Ok(None);
    };
    let Some(user) = repository::user::find_by_id(db, user_id).await? else {
        return Ok(None);
    };
    if !user.is_active {
        return Ok(None);
    }

    let permissions = crate::service::rbac::permissions_for_user(db, user.id).await?;
    if decoded.claims.role != role_for_permissions(&permissions) {
        return Ok(None);
    }

    Ok(Some(AuthenticatedUser {
        id: user.id,
        username: user.username,
        email: user.email,
        is_active: user.is_active,
        created_at: user.created_at,
        updated_at: user.updated_at,
        last_login_at: user.last_login_at,
        last_login_ip: user.last_login_ip,
        permissions: permissions.into_iter().collect(),
        access_token: token.to_owned(),
    }))
}

pub async fn signout(db: &DatabaseConnection, user_id: i32, refresh_token: &str) -> Result<(), DbErr> {
    repository::session::revoke_for_user(db, user_id, refresh_token).await
}

#[derive(Debug, thiserror::Error)]
pub enum RefreshError {
    #[error("invalid or expired refresh token")]
    InvalidToken,
    #[error("refresh token reuse detected")]
    ReuseDetected,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub async fn refresh(
    db: &DatabaseConnection,
    config: &Config,
    token: &str,
) -> Result<LoginResult, RefreshError> {
    let txn = db.begin().await.map_err(anyhow::Error::from)?;
    let session = repository::session::find_any(&txn, token)
        .await
        .map_err(anyhow::Error::from)?
        .ok_or(RefreshError::InvalidToken)?;
    let family_id = session.family_id.ok_or(RefreshError::InvalidToken)?;
    if session.used_at.is_some() {
        repository::session::revoke_family(&txn, &family_id)
            .await
            .map_err(anyhow::Error::from)?;
        txn.commit().await.map_err(anyhow::Error::from)?;
        return Err(RefreshError::ReuseDetected);
    }
    if session.revoked_at.is_some() || session.expires_at <= Utc::now() {
        return Err(RefreshError::InvalidToken);
    }
    let user = repository::user::find_by_id(&txn, session.user_id)
        .await
        .map_err(anyhow::Error::from)?
        .filter(|user| user.is_active)
        .ok_or(RefreshError::InvalidToken)?;
    if !repository::session::consume(&txn, token)
        .await
        .map_err(anyhow::Error::from)?
    {
        repository::session::revoke_family(&txn, &family_id)
            .await
            .map_err(anyhow::Error::from)?;
        txn.commit().await.map_err(anyhow::Error::from)?;
        return Err(RefreshError::ReuseDetected);
    }
    let refresh_token = new_refresh_token();
    let refresh_expires_at = Utc::now() + Duration::days(REFRESH_DAYS);
    repository::session::insert(&txn, &refresh_token, session.user_id, refresh_expires_at, &family_id)
        .await
        .map_err(anyhow::Error::from)?;
    txn.commit().await.map_err(anyhow::Error::from)?;
    let (access_token, expires_at) = issue_access_token(db, config, user.id).await?;
    Ok(LoginResult {
        user_id: user.id,
        last_login_at: user.last_login_at,
        token: access_token,
        expires_at,
        refresh_token,
        refresh_expires_at,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ChangePasswordError {
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub async fn change_password(
    db: &DatabaseConnection,
    user_id: i32,
    current_password: &str,
    new_password: &str,
) -> Result<(), ChangePasswordError> {
    let user = repository::user::find_by_id(db, user_id)
        .await
        .map_err(anyhow::Error::from)?
        .ok_or(ChangePasswordError::InvalidCredentials)?;
    if !service::user::verify_password(&user.password_hash, current_password).await {
        return Err(ChangePasswordError::InvalidCredentials);
    }
    let hash = service::user::hash_password(new_password).await?;
    let txn = db.begin().await.map_err(anyhow::Error::from)?;
    repository::user::update_password(&txn, user_id, hash)
        .await
        .map_err(anyhow::Error::from)?;
    repository::session::delete_all_for_user(&txn, user_id)
        .await
        .map_err(anyhow::Error::from)?;
    txn.commit().await.map_err(anyhow::Error::from)?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum PasswordResetError {
    #[error("too many requests; try again later")]
    Limited,
    #[error("invalid or expired reset token")]
    InvalidToken,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub async fn forgot_password(
    db: &DatabaseConnection,
    redis: &redis::aio::ConnectionManager,
    config: &Config,
    email: &str,
    ip: &str,
) -> Result<(), PasswordResetError> {
    if service::redis::request_limited(redis, "forgot-password", email, ip, 5)
        .await
        .map_err(anyhow::Error::from)?
    {
        return Err(PasswordResetError::Limited);
    }
    let email = email.trim().to_lowercase();
    let user = repository::user::find_by_email(db, &email)
        .await
        .map_err(anyhow::Error::from)?;
    if let Some(user) = user {
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let digest = format!("{:x}", Sha256::digest(token.as_bytes()));
        repository::password_reset::insert(
            db,
            &digest,
            user.id,
            Utc::now() + Duration::minutes(15),
        )
        .await
        .map_err(anyhow::Error::from)?;
        let body = format!("Use this password reset token within 15 minutes: {token}");
        if crate::utils::mail::send_mail_with_config(
            config.clone(),
            &email,
            "Password reset",
            &body,
        )
        .await
        .is_err()
        {
            tracing::error!("failed to send password reset email");
        }
    }
    Ok(())
}

pub async fn reset_password(
    db: &DatabaseConnection,
    token: &str,
    new_password: &str,
) -> Result<(), PasswordResetError> {
    let digest = format!("{:x}", Sha256::digest(token.as_bytes()));
    let hash = service::user::hash_password(new_password).await?;
    let txn = db.begin().await.map_err(anyhow::Error::from)?;
    let user_id = repository::password_reset::consume(&txn, &digest)
        .await
        .map_err(anyhow::Error::from)?
        .ok_or(PasswordResetError::InvalidToken)?;
    repository::user::update_password(&txn, user_id, hash)
        .await
        .map_err(anyhow::Error::from)?;
    repository::session::delete_all_for_user(&txn, user_id)
        .await
        .map_err(anyhow::Error::from)?;
    txn.commit().await.map_err(anyhow::Error::from)?;
    Ok(())
}
