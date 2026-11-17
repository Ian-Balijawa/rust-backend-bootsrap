use crate::{AppState, controller::health};
use axum::{Router, routing::get};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health_handler))
        .route("/ready", get(health::ready_handler))
        .route("/live", get(health::live_handler))
        .route("/app-info", get(health::config_handler))
}
