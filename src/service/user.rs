// User-related business logic

use crate::model::user;
use crate::repository;
use crate::scheduler::task_scheduler::TaskScheduler;
use bcrypt::{DEFAULT_COST, hash, verify};
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde_json::json;

// Register a new user
pub async fn register_user(
    db: &DatabaseConnection,
    scheduler: &TaskScheduler,
    username: &str,
    password: &str,
    email: &str,
) -> anyhow::Result<user::Model> {
    // Check if the username already exists
    let exists = repository::user::find_by_username(db, username)
        .await?
        .is_some();
    if exists {
        anyhow::bail!("Username already exists");
    }
    let email_exists = repository::user::find_by_email(db, email).await?.is_some();
    if email_exists {
        anyhow::bail!("Email already exists");
    }
    let password_hash = hash(password, DEFAULT_COST)?;
    let user = repository::user::insert(db, username, password_hash, Some(email)).await?;
    crate::service::rbac::assign_registration_role(db, user.id).await?;
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
) -> anyhow::Result<(user::Model, String, DateTime<Utc>)> {
    let user = repository::user::record_login(db, user, ip).await?;
    let (token, expires_at) = crate::service::auth::create_session(db, user.id).await?;
    Ok((user, token, expires_at))
}

// Verify password
pub fn verify_password(hash: &str, password: &str) -> bool {
    verify(password, hash).unwrap_or(false)
}
