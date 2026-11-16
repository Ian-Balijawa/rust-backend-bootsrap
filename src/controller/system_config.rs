use crate::{
    AppState,
    http::{
        auth::{Required, SystemConfigRead, SystemConfigWrite},
        response::{ApiError, ApiResponse},
    },
    model::system_config,
    repository,
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct ConfigContent {
    content: String,
}

pub(crate) async fn list_handler(
    State(state): State<AppState>,
    _permission: Required<SystemConfigRead>,
) -> Result<Json<ApiResponse<Vec<system_config::Model>>>, ApiError> {
    Ok(Json(ApiResponse::ok(
        repository::system_config::list(&state.db)
            .await
            .map_err(ApiError::internal)?,
    )))
}

pub(crate) async fn get_handler(
    State(state): State<AppState>,
    _permission: Required<SystemConfigRead>,
    Path(key): Path<String>,
) -> Result<Json<ApiResponse<system_config::Model>>, ApiError> {
    let config = repository::system_config::get(&state.db, &key)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("system config not found"))?;
    Ok(Json(ApiResponse::ok(config)))
}

pub(crate) async fn create_handler(
    State(state): State<AppState>,
    _permission: Required<SystemConfigWrite>,
    Path(key): Path<String>,
    Json(payload): Json<ConfigContent>,
) -> Result<Json<ApiResponse<system_config::Model>>, ApiError> {
    if key.trim().is_empty() {
        return Err(ApiError::bad_request("key is required"));
    }
    let config = repository::system_config::set(&state.db, key, payload.content)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ApiResponse::ok(config)))
}

pub(crate) async fn update_handler(
    State(state): State<AppState>,
    _permission: Required<SystemConfigWrite>,
    Path(key): Path<String>,
    Json(payload): Json<ConfigContent>,
) -> Result<Json<ApiResponse<system_config::Model>>, ApiError> {
    let config = repository::system_config::update(&state.db, &key, payload.content)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("system config not found"))?;
    Ok(Json(ApiResponse::ok(config)))
}

pub(crate) async fn delete_handler(
    State(state): State<AppState>,
    _permission: Required<SystemConfigWrite>,
    Path(key): Path<String>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    if repository::system_config::delete(&state.db, &key)
        .await
        .map_err(ApiError::internal)?
    {
        Ok(Json(ApiResponse::message("system config deleted")))
    } else {
        Err(ApiError::not_found("system config not found"))
    }
}
