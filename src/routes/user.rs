use crate::{AppState, controller::user};
use axum::{
    Router,
    routing::{get, post, put},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(user::login_handler))
        .route("/register", post(user::register_handler))
        .route("/me", get(user::me_handler))
        .route("/logout", post(user::logout_handler))
        .route("/{id}/role", put(user::replace_role_handler))
}
