use crate::{
    AppState,
    http::{
        auth::{Required, TaskRead, TaskWrite},
        response::{ApiError, ApiResponse},
    },
    repository,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(crate) struct TaskQuery {
    user_id: Option<i32>,
}

#[derive(Deserialize)]
pub(crate) struct CreateTaskRequest {
    user_id: Option<i32>,
    action: String,
    schedule_time: DateTime<Utc>,
}

#[derive(Deserialize)]
pub(crate) struct UpdateTaskRequest {
    action: Option<String>,
    schedule_time: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub(crate) struct TaskResponse {
    id: i32,
    user_id: i32,
    action: String,
    schedule_time: DateTime<Utc>,
}

impl From<crate::model::task::Model> for TaskResponse {
    fn from(task: crate::model::task::Model) -> Self {
        Self {
            id: task.id,
            user_id: task.user_id,
            action: task.action,
            schedule_time: task.schedule_time,
        }
    }
}

pub(crate) async fn list_tasks_handler(
    State(state): State<AppState>,
    user: Required<TaskRead>,
    Query(query): Query<TaskQuery>,
) -> Result<Json<ApiResponse<Vec<TaskResponse>>>, ApiError> {
    let requested_user = query.user_id.unwrap_or(user.id);
    if requested_user != user.id && !user.has_permission("user:manage") {
        return Err(ApiError::forbidden("cannot read another user's tasks"));
    }
    let tasks = if query.user_id.is_some() || !user.has_permission("user:manage") {
        repository::task::get_tasks_by_user(&state.db, requested_user).await
    } else {
        repository::task::get_all_tasks(&state.db).await
    }
    .map_err(ApiError::internal)?;

    Ok(Json(ApiResponse::ok(
        tasks.into_iter().map(TaskResponse::from).collect(),
    )))
}

pub(crate) async fn get_task_handler(
    State(state): State<AppState>,
    user: Required<TaskRead>,
    Path(id): Path<i32>,
) -> Result<Json<ApiResponse<TaskResponse>>, ApiError> {
    let task = repository::task::get_task_by_id(&state.db, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("task not found"))?;
    ensure_task_access(&user, task.user_id)?;

    Ok(Json(ApiResponse::ok(TaskResponse::from(task))))
}

pub(crate) async fn create_task_handler(
    State(state): State<AppState>,
    user: Required<TaskWrite>,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<Json<ApiResponse<TaskResponse>>, ApiError> {
    if payload.action.trim().is_empty() {
        return Err(ApiError::bad_request("action is required"));
    }

    let user_id = payload.user_id.unwrap_or(user.id);
    ensure_task_access(&user, user_id)?;
    let task = repository::task::add_task(
        &state.db,
        user_id,
        payload.action.trim(),
        payload.schedule_time,
    )
    .await
    .map_err(ApiError::internal)?;

    Ok(Json(ApiResponse::ok(TaskResponse::from(task))))
}

pub(crate) async fn update_task_handler(
    State(state): State<AppState>,
    user: Required<TaskWrite>,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateTaskRequest>,
) -> Result<Json<ApiResponse<TaskResponse>>, ApiError> {
    if matches!(payload.action.as_deref(), Some(action) if action.trim().is_empty()) {
        return Err(ApiError::bad_request("action cannot be empty"));
    }
    let existing = repository::task::get_task_by_id(&state.db, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("task not found"))?;
    ensure_task_access(&user, existing.user_id)?;

    let task = repository::task::update_task(
        &state.db,
        id,
        payload.action.map(|action| action.trim().to_string()),
        payload.schedule_time,
    )
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(|| ApiError::not_found("task not found"))?;

    Ok(Json(ApiResponse::ok(TaskResponse::from(task))))
}

pub(crate) async fn delete_task_handler(
    State(state): State<AppState>,
    user: Required<TaskWrite>,
    Path(id): Path<i32>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    let existing = repository::task::get_task_by_id(&state.db, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("task not found"))?;
    ensure_task_access(&user, existing.user_id)?;
    if repository::task::delete_task(&state.db, id)
        .await
        .map_err(ApiError::internal)?
    {
        Ok(Json(ApiResponse::message("task deleted")))
    } else {
        Err(ApiError::not_found("task not found"))
    }
}

fn ensure_task_access<P>(user: &Required<P>, owner_id: i32) -> Result<(), ApiError> {
    if user.id == owner_id || user.has_permission("user:manage") {
        Ok(())
    } else {
        Err(ApiError::forbidden("cannot access another user's task"))
    }
}
