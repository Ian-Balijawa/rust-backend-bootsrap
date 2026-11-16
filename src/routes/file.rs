use crate::{AppState, controller::file};
use axum::{
    Router,
    routing::{get, post},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(file::upload))
        .route("/{id}", get(file::metadata).delete(file::delete))
        .route("/{id}/download", get(file::download))
}
