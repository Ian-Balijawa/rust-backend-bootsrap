use crate::{AppState, controller::note};
use axum::{Router, routing::get};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(note::list).post(note::create))
        .route(
            "/{id}",
            get(note::get).patch(note::update).delete(note::delete),
        )
}
