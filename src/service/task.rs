use crate::{model::task, repository};
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};

#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("task not found")]
    NotFound,
    #[error("action must be between 1 and 500 characters")]
    InvalidAction,
    #[error(transparent)]
    Database(#[from] DbErr),
}

fn checked_action(action: &str) -> Result<&str, TaskError> {
    let action = action.trim();
    if action.is_empty() || action.len() > 500 {
        Err(TaskError::InvalidAction)
    } else {
        Ok(action)
    }
}

pub async fn create(
    db: &DatabaseConnection,
    owner_id: i32,
    action: &str,
    schedule_time: DateTime<Utc>,
) -> Result<task::Model, TaskError> {
    Ok(repository::task::add_task(db, owner_id, checked_action(action)?, schedule_time).await?)
}

pub async fn list(
    db: &DatabaseConnection,
    owner_id: i32,
    page: u64,
    per_page: u64,
) -> Result<(Vec<task::Model>, u64), TaskError> {
    Ok(repository::task::list_for_owner(db, owner_id, page, per_page).await?)
}

pub async fn get(
    db: &DatabaseConnection,
    owner_id: i32,
    id: i32,
) -> Result<task::Model, TaskError> {
    repository::task::find_for_owner(db, owner_id, id)
        .await?
        .ok_or(TaskError::NotFound)
}

pub async fn update(
    db: &DatabaseConnection,
    owner_id: i32,
    id: i32,
    action: Option<String>,
    schedule_time: Option<DateTime<Utc>>,
) -> Result<task::Model, TaskError> {
    let action = action
        .as_deref()
        .map(checked_action)
        .transpose()?
        .map(str::to_owned);
    repository::task::update_for_owner(db, owner_id, id, action, schedule_time)
        .await?
        .ok_or(TaskError::NotFound)
}

pub async fn delete(db: &DatabaseConnection, owner_id: i32, id: i32) -> Result<(), TaskError> {
    if repository::task::soft_delete_for_owner(db, owner_id, id).await? {
        Ok(())
    } else {
        Err(TaskError::NotFound)
    }
}
