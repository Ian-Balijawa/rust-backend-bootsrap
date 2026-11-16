use crate::{AppState, controller::user};
use axum::{
    Router,
    routing::{get, post, put},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/signin", post(user::login_handler))
        .route("/refresh", post(user::refresh_handler))
        .route("/signup", post(user::register_handler))
        .route("/me", get(user::me_handler))
        .route("/signout", post(user::logout_handler))
        .route("/change-password", post(user::change_password_handler))
        .route("/forgot-password", post(user::forgot_password_handler))
        .route("/reset-password", post(user::reset_password_handler))
        .route("/{id}/role", put(user::replace_role_handler))
}
