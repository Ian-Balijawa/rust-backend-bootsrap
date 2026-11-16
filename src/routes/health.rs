use crate::{AppState, controller::health};
use axum::{Router, routing::get};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health_handler))
        .route("/ready", get(health::health_handler))
        .route("/live", get(|| async { "ok" }))
        .route("/app-info", get(health::config_handler))
}
