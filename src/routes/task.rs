use crate::{AppState, controller::task};
use axum::{Router, routing::get};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            get(task::list_tasks_handler).post(task::create_task_handler),
        )
        .route(
            "/{id}",
            get(task::get_task_handler)
                .patch(task::update_task_handler)
                .delete(task::delete_task_handler),
        )
}
