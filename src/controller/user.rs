use crate::{
    AppState,
    http::{
        auth::{Required, UserManage},
        response::{ApiError, ApiResponse},
    },
    service,
    service::auth::AuthenticatedUser,
};
use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, header::USER_AGENT},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Deserialize)]
pub(crate) struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
pub(crate) struct LoginResponse {
    token: String,
    token_type: &'static str,
    expires_at: DateTime<Utc>,
    user_id: i32,
    last_login_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub(crate) struct RegisterRequest {
    username: String,
    password: String,
    email: String,
}

#[derive(Serialize)]
pub(crate) struct CurrentUserResponse {
    id: i32,
    username: String,
    email: Option<String>,
    is_active: bool,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    last_login_at: Option<DateTime<Utc>>,
    last_login_ip: Option<String>,
    permissions: Vec<String>,
}

#[derive(Deserialize)]
pub(crate) struct ReplaceRoleRequest {
    role: String,
}

pub(crate) async fn login_handler(
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, ApiError> {
    let ip = remote_addr.ip().to_string();
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok());
    let result = service::auth::login(
        &state.db,
        &state.redis,
        &payload.username,
        &payload.password,
        &ip,
        user_agent,
    )
    .await
    .map_err(|error| match error {
        service::auth::LoginError::Limited => ApiError {
            status: axum::http::StatusCode::TOO_MANY_REQUESTS,
            message: error.to_string(),
        },
        service::auth::LoginError::InvalidCredentials => ApiError::unauthorized(error.to_string()),
        service::auth::LoginError::Disabled => ApiError::forbidden(error.to_string()),
        service::auth::LoginError::Internal(error) => ApiError::internal(error),
    })?;
    Ok(Json(ApiResponse::ok(LoginResponse {
        token: result.token,
        token_type: "Bearer",
        expires_at: result.expires_at,
        user_id: result.user_id,
        last_login_at: result.last_login_at,
    })))
}

pub(crate) async fn register_handler(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    if payload.username.trim().is_empty() || payload.password.len() < 8 {
        return Err(ApiError::bad_request(
            "username is required and password must contain at least 8 characters",
        ));
    }
    if !payload
        .email
        .trim()
        .split_once('@')
        .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'))
    {
        return Err(ApiError::bad_request("a valid email is required"));
    }
    service::user::register_user(
        &state.db,
        &state.scheduler,
        payload.username.trim(),
        &payload.password,
        payload.email.trim(),
    )
    .await
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(ApiResponse::message("registration successful")))
}

pub(crate) async fn me_handler(user: AuthenticatedUser) -> Json<ApiResponse<CurrentUserResponse>> {
    let mut permissions: Vec<_> = user.permissions.iter().cloned().collect();
    permissions.sort();
    Json(ApiResponse::ok(CurrentUserResponse {
        id: user.id,
        username: user.username,
        email: user.email,
        is_active: user.is_active,
        created_at: user.created_at,
        updated_at: user.updated_at,
        last_login_at: user.last_login_at,
        last_login_ip: user.last_login_ip,
        permissions,
    }))
}

pub(crate) async fn logout_handler(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    service::auth::delete_session(&state.db, &user.session_token)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ApiResponse::message("logged out")))
}

pub(crate) async fn replace_role_handler(
    State(state): State<AppState>,
    _permission: Required<UserManage>,
    Path(user_id): Path<i32>,
    Json(payload): Json<ReplaceRoleRequest>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    if payload.role.trim().is_empty() {
        return Err(ApiError::bad_request("role is required"));
    }
    service::rbac::replace_role(&state.db, user_id, payload.role.trim())
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(ApiResponse::message("user role updated")))
}
