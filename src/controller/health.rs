use crate::{
    AppState,
    http::{
        auth::{Required, SystemConfigRead},
        response::{ApiError, ApiResponse},
    },
};
use axum::{Json, extract::State};
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct HealthResponse {
    status: &'static str,
    started_at: DateTime<Utc>,
    scheduler_running: bool,
    scheduler_max_concurrent_tasks: usize,
}

#[derive(Serialize)]
pub(crate) struct ConfigResponse {
    database: String,
    redis: String,
    max_upload_bytes: u64,
    smtp_server: String,
    smtp_port: u16,
    smtp_configured: bool,
    log_level: String,
}

fn database_kind(db_url: &str) -> String {
    db_url
        .split_once(':')
        .map(|(kind, _)| kind.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) async fn health_handler(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<HealthResponse>>, ApiError> {
    state.db.ping().await.map_err(|error| {
        tracing::error!(%error, "database health check failed");
        ApiError {
            status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
            message: "database unavailable".to_string(),
        }
    })?;
    let mut redis = state.redis.clone();
    let _: String = redis::cmd("PING")
        .query_async(&mut redis)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Redis health check failed");
            ApiError {
                status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
                message: "Redis unavailable".to_string(),
            }
        })?;
    Ok(Json(ApiResponse::ok(HealthResponse {
        status: "ok",
        started_at: state.started_at,
        scheduler_running: state.scheduler.is_running().await,
        scheduler_max_concurrent_tasks: state.scheduler.max_concurrent_tasks(),
    })))
}

pub(crate) async fn config_handler(
    State(state): State<AppState>,
    _permission: Required<SystemConfigRead>,
) -> Json<ApiResponse<ConfigResponse>> {
    Json(ApiResponse::ok(ConfigResponse {
        database: database_kind(&state.config.db_url),
        redis: database_kind(&state.config.redis_url),
        max_upload_bytes: state.config.max_upload_bytes,
        smtp_server: state.config.smtp_server,
        smtp_port: state.config.smtp_port,
        smtp_configured: !state.config.smtp_user.trim().is_empty()
            && !state.config.smtp_pass.trim().is_empty(),
        log_level: state.config.log_level,
    }))
}
