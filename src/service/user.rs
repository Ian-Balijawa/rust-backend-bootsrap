// User-related business logic

use crate::model::user;
use crate::repository;
use crate::scheduler::task_scheduler::TaskScheduler;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use sea_orm::{DatabaseConnection, DbErr, TransactionTrait};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum RegisterError {
    #[error("username is required")]
    InvalidUsername,
    #[error("username already exists")]
    DuplicateUsername,
    #[error("email already exists")]
    DuplicateEmail,
    #[error("too many signup attempts; try again later")]
    Limited,
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

// Register a new user
pub async fn register_user(
    db: &DatabaseConnection,
    redis: &redis::aio::ConnectionManager,
    scheduler: &TaskScheduler,
    username: &str,
    password: &str,
    email: &str,
    ip: &str,
) -> Result<user::Model, RegisterError> {
    if username.trim().is_empty() {
        return Err(RegisterError::InvalidUsername);
    }
    if crate::service::redis::request_limited(redis, "signup", email, ip, 5)
        .await
        .map_err(anyhow::Error::from)?
    {
        return Err(RegisterError::Limited);
    }
    // Check if the username already exists
    let exists = repository::user::find_by_username(db, username)
        .await?
        .is_some();
    if exists {
        return Err(RegisterError::DuplicateUsername);
    }
    let email = email.trim().to_lowercase();
    let email_exists = repository::user::find_by_email(db, &email).await?.is_some();
    if email_exists {
        return Err(RegisterError::DuplicateEmail);
    }
    let password_hash = hash_password(password).await?;
    let txn = db.begin().await?;
    let user = repository::user::insert(&txn, username, password_hash, Some(&email)).await?;
    crate::service::rbac::assign_registration_role(&txn, user.id).await?;
    txn.commit().await?;
    // Send mail asynchronously through the scheduler
    let mail_data = json!({
        "to": email,
        "subject": "Successful registration",
        "body": "Welcome to register!",
    });
    let _ = scheduler
        .add_one_time_task(
            "User registration email".to_string(),
            "mail_sender".to_string(),
            Some(mail_data),
            Some(60), // Timeout 60 seconds
        )
        .await;
    Ok(user)
}

pub async fn record_login(
    db: &DatabaseConnection,
    user: user::Model,
    ip: &str,
) -> anyhow::Result<user::Model> {
    let user = repository::user::record_login(db, user, ip).await?;
    Ok(user)
}

// Verify password
pub async fn hash_password(password: &str) -> anyhow::Result<String> {
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|hash| hash.to_string())
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    })
    .await?
}

pub async fn verify_password(hash: &str, password: &str) -> bool {
    let hash = hash.to_owned();
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || {
        if hash.starts_with("$argon2id$") {
            PasswordHash::new(&hash).ok().is_some_and(|parsed| {
                Argon2::default()
                    .verify_password(password.as_bytes(), &parsed)
                    .is_ok()
            })
        } else if hash.starts_with("$2") {
            // Existing bcrypt accounts remain usable while their hashes are upgraded on login.
            bcrypt::verify(password, &hash).unwrap_or(false)
        } else {
            false
        }
    })
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn passwords_use_argon2id_and_random_salts() {
        let first = hash_password("correct horse battery staple").await.unwrap();
        let second = hash_password("correct horse battery staple").await.unwrap();
        assert!(first.starts_with("$argon2id$"));
        assert_ne!(first, second);
        assert!(verify_password(&first, "correct horse battery staple").await);
        assert!(!verify_password(&first, "incorrect").await);
    }
}
