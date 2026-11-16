use crate::{AppState, controller::scheduler};
use axum::{
    Router,
    routing::{get, post},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/tasks",
            get(scheduler::list_tasks_handler).post(scheduler::create_task_handler),
        )
        .route(
            "/tasks/{id}",
            get(scheduler::get_task_handler).delete(scheduler::delete_task_handler),
        )
        .route(
            "/tasks/{id}/executions",
            get(scheduler::list_task_executions_handler),
        )
        .route("/tasks/{id}/run", post(scheduler::run_task_handler))
}
