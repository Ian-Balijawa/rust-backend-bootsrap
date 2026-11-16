use crate::{
    AppState,
    http::{
        auth::{Required, UserManage},
        response::{ApiError, ApiResponse},
        validated_json::ValidatedJson,
    },
    service,
    service::auth::AuthenticatedUser,
};
use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode, header::USER_AGENT},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use validator::Validate;

fn deserialize_email<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let email = String::deserialize(deserializer)?;
    Ok(email.trim().to_lowercase())
}

#[derive(Deserialize, Validate)]
pub(crate) struct LoginRequest {
    #[serde(deserialize_with = "deserialize_email")]
    #[validate(email)]
    email: String,
    #[validate(length(min = 1))]
    password: String,
}

#[derive(Serialize)]
pub(crate) struct LoginResponse {
    token: String,
    token_type: &'static str,
    expires_at: DateTime<Utc>,
    refresh_token: String,
    refresh_expires_at: DateTime<Utc>,
    user_id: i32,
    last_login_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize, Validate)]
pub(crate) struct RegisterRequest {
    #[validate(length(min = 1, max = 100))]
    username: String,
    #[validate(length(min = 8, max = 1024))]
    password: String,
    #[serde(deserialize_with = "deserialize_email")]
    #[validate(email)]
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

#[derive(Deserialize, Validate)]
pub(crate) struct ChangePasswordRequest {
    #[validate(length(min = 1))]
    current_password: String,
    #[validate(length(min = 8, max = 1024))]
    new_password: String,
}

#[derive(Deserialize, Validate)]
pub(crate) struct ForgotPasswordRequest {
    #[serde(deserialize_with = "deserialize_email")]
    #[validate(email)]
    email: String,
}

#[derive(Deserialize, Validate)]
pub(crate) struct ResetPasswordRequest {
    #[validate(length(equal = 64))]
    token: String,
    #[validate(length(min = 8, max = 1024))]
    new_password: String,
}

#[derive(Deserialize, Validate)]
pub(crate) struct RefreshRequest {
    #[validate(length(equal = 64))]
    refresh_token: String,
}

pub(crate) async fn login_handler(
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, ApiError> {
    let ip = remote_addr.ip().to_string();
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok());
    let result = service::auth::login(
        &state.db,
        &state.redis,
        &state.config,
        &payload.email,
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
        service::auth::LoginError::Internal(error) => ApiError::internal(error),
    })?;
    Ok(Json(ApiResponse::ok(LoginResponse {
        token: result.token,
        token_type: "Bearer",
        expires_at: result.expires_at,
        refresh_token: result.refresh_token,
        refresh_expires_at: result.refresh_expires_at,
        user_id: result.user_id,
        last_login_at: result.last_login_at,
    })))
}

pub(crate) async fn change_password_handler(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    ValidatedJson(payload): ValidatedJson<ChangePasswordRequest>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    service::auth::change_password(
        &state.db,
        user.id,
        &payload.current_password,
        &payload.new_password,
    )
    .await
    .map_err(|error| match error {
        service::auth::ChangePasswordError::InvalidCredentials => {
            ApiError::unauthorized("Invalid credentials")
        }
        service::auth::ChangePasswordError::Internal(error) => ApiError::internal(error),
    })?;
    Ok(Json(ApiResponse::message("password changed")))
}

pub(crate) async fn forgot_password_handler(
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<ForgotPasswordRequest>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    service::auth::forgot_password(
        &state.db,
        &state.redis,
        &state.config,
        &payload.email,
        &remote_addr.ip().to_string(),
    )
    .await
    .map_err(|error| match error {
        service::auth::PasswordResetError::Limited => ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: error.to_string(),
        },
        service::auth::PasswordResetError::InvalidToken => ApiError::bad_request(error.to_string()),
        service::auth::PasswordResetError::Internal(error) => ApiError::internal(error),
    })?;
    Ok(Json(ApiResponse::message(
        "If the account exists, a reset email has been sent",
    )))
}

pub(crate) async fn reset_password_handler(
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<ResetPasswordRequest>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    let limited = service::redis::request_limited(
        &state.redis,
        "reset-password",
        &payload.token,
        &remote_addr.ip().to_string(),
        10,
    )
    .await
    .map_err(ApiError::internal)?;
    if limited {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "too many requests; try again later".to_string(),
        });
    }
    service::auth::reset_password(&state.db, &payload.token, &payload.new_password)
        .await
        .map_err(|error| match error {
            service::auth::PasswordResetError::InvalidToken => {
                ApiError::bad_request(error.to_string())
            }
            service::auth::PasswordResetError::Limited => ApiError::bad_request(error.to_string()),
            service::auth::PasswordResetError::Internal(error) => ApiError::internal(error),
        })?;
    Ok(Json(ApiResponse::message("password reset")))
}

pub(crate) async fn register_handler(
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<RegisterRequest>,
) -> Result<(StatusCode, Json<ApiResponse<()>>), ApiError> {
    service::user::register_user(
        &state.db,
        &state.redis,
        &state.scheduler,
        payload.username.trim(),
        &payload.password,
        payload.email.trim(),
        &remote_addr.ip().to_string(),
    )
    .await
    .map_err(|error| match error {
        service::user::RegisterError::InvalidUsername => {
            ApiError::bad_request("username is required")
        }
        service::user::RegisterError::DuplicateUsername => {
            ApiError::conflict("username already exists")
        }
        service::user::RegisterError::DuplicateEmail => ApiError::conflict("email already exists"),
        service::user::RegisterError::Limited => ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: error.to_string(),
        },
        service::user::RegisterError::Database(error) => {
            if matches!(
                error.sql_err(),
                Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
            ) {
                ApiError::conflict("account already exists")
            } else {
                ApiError::internal(error)
            }
        }
        service::user::RegisterError::Internal(error) => ApiError::internal(error),
    })?;
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::message("registration successful")),
    ))
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
    ValidatedJson(payload): ValidatedJson<RefreshRequest>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    service::auth::signout(&state.db, user.id, &payload.refresh_token)
        .await
        .map_err(ApiError::internal)?;
    service::redis::revoke_access(&state.redis, &user.access_token)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ApiResponse::message("logged out")))
}

pub(crate) async fn refresh_handler(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<RefreshRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, ApiError> {
    let result = service::auth::refresh(&state.db, &state.config, &payload.refresh_token)
        .await
        .map_err(|error| match error {
            service::auth::RefreshError::InvalidToken | service::auth::RefreshError::ReuseDetected =>
                ApiError::unauthorized("invalid or expired refresh token"),
            service::auth::RefreshError::Internal(error) => ApiError::internal(error),
        })?;
    Ok(Json(ApiResponse::ok(LoginResponse {
        token: result.token,
        token_type: "Bearer",
        expires_at: result.expires_at,
        refresh_token: result.refresh_token,
        refresh_expires_at: result.refresh_expires_at,
        user_id: result.user_id,
        last_login_at: result.last_login_at,
    })))
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
