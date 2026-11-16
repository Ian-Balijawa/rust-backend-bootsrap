pub mod file;
pub mod health;
pub mod scheduler;
pub mod system_config;
pub mod task;
pub mod user;

use crate::AppState;
use axum::Router;
use axum::http::StatusCode;

pub async fn fallback() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

pub fn create_router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .nest("/api/v1/auth", user::router())
        .nest("/api/v1/tasks", task::router())
        .nest("/api/v1/files", file::router())
        .nest("/api/v1/scheduler", scheduler::router())
        .nest("/api/v1/system-config", system_config::router())
        .fallback(fallback)
}
