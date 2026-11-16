use crate::{AppState, controller::system_config};
use axum::{Router, routing::get};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(system_config::list_handler))
        .route(
            "/{key}",
            get(system_config::get_handler)
                .post(system_config::create_handler)
                .put(system_config::update_handler)
                .delete(system_config::delete_handler),
        )
}
