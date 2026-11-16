use crate::{
    AppState,
    http::{
        auth::{Required, TaskRead, TaskWrite},
        response::{ApiError, ApiResponse},
    },
    service,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(crate) struct TaskQuery {
    #[serde(default = "default_page")]
    page: u64,
    #[serde(default = "default_per_page")]
    per_page: u64,
}

fn default_page() -> u64 {
    1
}
fn default_per_page() -> u64 {
    20
}

#[derive(Deserialize)]
pub(crate) struct CreateTaskRequest {
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
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<crate::model::task::Model> for TaskResponse {
    fn from(task: crate::model::task::Model) -> Self {
        Self {
            id: task.id,
            user_id: task.user_id,
            action: task.action,
            schedule_time: task.schedule_time,
            created_at: task.created_at,
            updated_at: task.updated_at,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct PageMeta {
    page: u64,
    per_page: u64,
    total: u64,
    total_pages: u64,
}

#[derive(Serialize)]
pub(crate) struct TaskPage {
    items: Vec<TaskResponse>,
    meta: PageMeta,
}

fn task_error(error: service::task::TaskError) -> ApiError {
    match error {
        service::task::TaskError::NotFound => ApiError::not_found("task not found"),
        service::task::TaskError::InvalidAction => ApiError::bad_request(error.to_string()),
        service::task::TaskError::Database(error) => ApiError::internal(error),
    }
}

pub(crate) async fn list_tasks_handler(
    State(state): State<AppState>,
    user: Required<TaskRead>,
    Query(query): Query<TaskQuery>,
) -> Result<Json<ApiResponse<TaskPage>>, ApiError> {
    if query.page == 0 || query.per_page == 0 || query.per_page > 100 {
        return Err(ApiError::bad_request(
            "page must be positive and per_page must be 1..=100",
        ));
    }
    let (tasks, total) = service::task::list(&state.db, user.id, query.page, query.per_page)
        .await
        .map_err(task_error)?;
    Ok(Json(ApiResponse::ok(TaskPage {
        items: tasks.into_iter().map(TaskResponse::from).collect(),
        meta: PageMeta {
            page: query.page,
            per_page: query.per_page,
            total,
            total_pages: total.div_ceil(query.per_page),
        },
    })))
}

pub(crate) async fn get_task_handler(
    State(state): State<AppState>,
    user: Required<TaskRead>,
    Path(id): Path<i32>,
) -> Result<Json<ApiResponse<TaskResponse>>, ApiError> {
    let task = service::task::get(&state.db, user.id, id)
        .await
        .map_err(task_error)?;
    Ok(Json(ApiResponse::ok(task.into())))
}

pub(crate) async fn create_task_handler(
    State(state): State<AppState>,
    user: Required<TaskWrite>,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<ApiResponse<TaskResponse>>), ApiError> {
    let task = service::task::create(&state.db, user.id, &payload.action, payload.schedule_time)
        .await
        .map_err(task_error)?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(task.into()))))
}

pub(crate) async fn update_task_handler(
    State(state): State<AppState>,
    user: Required<TaskWrite>,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateTaskRequest>,
) -> Result<Json<ApiResponse<TaskResponse>>, ApiError> {
    let task = service::task::update(
        &state.db,
        user.id,
        id,
        payload.action,
        payload.schedule_time,
    )
    .await
    .map_err(task_error)?;
    Ok(Json(ApiResponse::ok(task.into())))
}

pub(crate) async fn delete_task_handler(
    State(state): State<AppState>,
    user: Required<TaskWrite>,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    service::task::delete(&state.db, user.id, id)
        .await
        .map_err(task_error)?;
    Ok(StatusCode::NO_CONTENT)
}
