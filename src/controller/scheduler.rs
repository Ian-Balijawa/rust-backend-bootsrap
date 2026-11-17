use crate::{
    AppState,
    http::{
        auth::{Required, SchedulerRead, SchedulerWrite},
        response::{ApiError, ApiResponse},
    },
};
use axum::{
    Json,
    extract::{Path, State},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SchedulerTaskKind {
    OneTime,
    Recurring,
    Scheduled,
    Persistent,
}

#[derive(Deserialize)]
pub(crate) struct CreateSchedulerTaskRequest {
    name: String,
    executor_type: String,
    task_type: SchedulerTaskKind,
    data: Option<serde_json::Value>,
    interval_seconds: Option<u64>,
    next_run: Option<DateTime<Utc>>,
    timeout_seconds: Option<u64>,
    max_retries: Option<u32>,
}

#[utoipa::path(get, path = "/api/v1/scheduler/tasks", tag = "scheduler", security(("bearerAuth" = [])), responses((status = 200, description = "List scheduled tasks")))]
pub(crate) async fn list_tasks_handler(
    State(state): State<AppState>,
    _permission: Required<SchedulerRead>,
) -> Json<ApiResponse<Vec<crate::scheduler::task_scheduler::TaskInfo>>> {
    let tasks = state
        .scheduler
        .get_all_tasks()
        .await
        .into_iter()
        .map(|task| task.to_info())
        .collect();
    Json(ApiResponse::ok(tasks))
}

#[utoipa::path(get, path = "/api/v1/scheduler/tasks/{id}", tag = "scheduler", security(("bearerAuth" = [])), responses((status = 200, description = "Get scheduled task")))]
pub(crate) async fn get_task_handler(
    State(state): State<AppState>,
    _permission: Required<SchedulerRead>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<crate::scheduler::task_scheduler::TaskInfo>>, ApiError> {
    let task = state
        .scheduler
        .get_task(&id)
        .await
        .ok_or_else(|| ApiError::not_found("scheduler task not found"))?;
    Ok(Json(ApiResponse::ok(task.to_info())))
}

#[utoipa::path(get, path = "/api/v1/scheduler/tasks/{id}/executions", tag = "scheduler", security(("bearerAuth" = [])), responses((status = 200, description = "List task executions")))]
pub(crate) async fn list_task_executions_handler(
    State(state): State<AppState>,
    _permission: Required<SchedulerRead>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<crate::model::task_execution::Model>>>, ApiError> {
    let exists = crate::repository::scheduler::task_exists(&state.db, &id)
        .await
        .map_err(ApiError::internal)?;
    if !exists {
        return Err(ApiError::not_found("scheduler task not found"));
    }
    let executions = crate::repository::scheduler::list_executions(&state.db, &id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ApiResponse::ok(executions)))
}

#[utoipa::path(post, path = "/api/v1/scheduler/tasks", tag = "scheduler", security(("bearerAuth" = [])), responses((status = 200, description = "Create scheduled task")))]
pub(crate) async fn create_task_handler(
    State(state): State<AppState>,
    _permission: Required<SchedulerWrite>,
    Json(payload): Json<CreateSchedulerTaskRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    if payload.name.trim().is_empty() || payload.executor_type.trim().is_empty() {
        return Err(ApiError::bad_request("name and executor_type are required"));
    }

    let task_id = match payload.task_type {
        SchedulerTaskKind::OneTime => {
            state
                .scheduler
                .add_one_time_task(
                    payload.name,
                    payload.executor_type,
                    payload.data,
                    payload.timeout_seconds,
                )
                .await
        }
        SchedulerTaskKind::Recurring => {
            let interval_seconds = payload
                .interval_seconds
                .ok_or_else(|| ApiError::bad_request("interval_seconds is required"))?;
            if interval_seconds == 0 {
                return Err(ApiError::bad_request(
                    "interval_seconds must be greater than zero",
                ));
            }
            state
                .scheduler
                .add_recurring_task(
                    payload.name,
                    interval_seconds,
                    payload.executor_type,
                    payload.data,
                    payload.timeout_seconds,
                    payload.max_retries.unwrap_or(0),
                )
                .await
        }
        SchedulerTaskKind::Scheduled => {
            let next_run = payload
                .next_run
                .ok_or_else(|| ApiError::bad_request("next_run is required"))?;
            state
                .scheduler
                .add_scheduled_task(
                    payload.name,
                    next_run,
                    payload.executor_type,
                    payload.data,
                    payload.timeout_seconds,
                )
                .await
        }
        SchedulerTaskKind::Persistent => {
            state
                .scheduler
                .add_persistent_task(payload.name, payload.executor_type, payload.data)
                .await
        }
    }
    .map_err(ApiError::bad_request)?;

    Ok(Json(ApiResponse::ok(serde_json::json!({ "id": task_id }))))
}

#[utoipa::path(post, path = "/api/v1/scheduler/tasks/{id}/run", tag = "scheduler", security(("bearerAuth" = [])), responses((status = 200, description = "Run scheduled task")))]
pub(crate) async fn run_task_handler(
    State(state): State<AppState>,
    _permission: Required<SchedulerWrite>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    if state.scheduler.execute_task_now(&id).await {
        Ok(Json(ApiResponse::message("scheduler task executed")))
    } else {
        Err(ApiError::not_found(
            "scheduler task not found or execution failed",
        ))
    }
}

#[utoipa::path(delete, path = "/api/v1/scheduler/tasks/{id}", tag = "scheduler", security(("bearerAuth" = [])), responses((status = 200, description = "Delete scheduled task")))]
pub(crate) async fn delete_task_handler(
    State(state): State<AppState>,
    _permission: Required<SchedulerWrite>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    if state.scheduler.remove_task(&id).await {
        Ok(Json(ApiResponse::message("scheduler task deleted")))
    } else {
        Err(ApiError::not_found("scheduler task not found"))
    }
}
