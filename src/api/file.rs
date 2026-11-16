use crate::{
    AppState,
    api::{
        auth::{FileRead, FileWrite, Required},
        common::{ApiError, ApiResponse},
    },
    service,
};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use tokio::fs;
use tokio_util::io::ReaderStream;

fn upload_name(headers: &HeaderMap) -> Result<&str, ApiError> {
    let name = headers
        .get("x-file-name")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::bad_request("X-File-Name header is required"))?;
    if name.is_empty()
        || name.len() > 255
        || name.contains(['/', '\\'])
        || name.chars().any(char::is_control)
        || name == "."
        || name == ".."
    {
        return Err(ApiError::bad_request("invalid file name"));
    }
    Ok(name)
}

async fn upload(
    State(state): State<AppState>,
    user: Required<FileWrite>,
    request: Request,
) -> Result<impl IntoResponse, ApiError> {
    let name = upload_name(request.headers())?.to_string();
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    if content_type.len() > 255 {
        return Err(ApiError::bad_request("content type is too long"));
    }
    if request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|size| size > state.config.max_upload_bytes)
    {
        return Err(ApiError {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            message: "file exceeds configured size limit".to_string(),
        });
    }
    let model = service::file::save(
        &state.db,
        &state.config.upload_dir,
        user.id,
        &name,
        &content_type,
        state.config.max_upload_bytes,
        request.into_body(),
    )
    .await
    .map_err(|error| match error {
        service::file::UploadError::TooLarge => ApiError {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            message: error.to_string(),
        },
        service::file::UploadError::Stream(_) => ApiError::bad_request("invalid upload stream"),
        _ => ApiError::internal(error),
    })?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(model))))
}

async fn metadata(
    State(state): State<AppState>,
    user: Required<FileRead>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let model = authorized_file(&state, &user, &id).await?;
    Ok(Json(ApiResponse::ok(model)))
}

async fn download(
    State(state): State<AppState>,
    user: Required<FileRead>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let model = authorized_file(&state, &user, &id).await?;
    let path = service::file::path_for(&state.config.upload_dir, &model.id)
        .ok_or_else(|| ApiError::internal("invalid stored file id"))?;
    let file = fs::File::open(path).await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ApiError::not_found("file content not found")
        } else {
            ApiError::internal(error)
        }
    })?;
    let fallback: String = model
        .filename
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let disposition = format!(
        "attachment; filename=\"{fallback}\"; filename*=UTF-8''{}",
        utf8_percent_encode(&model.filename, NON_ALPHANUMERIC)
    );
    let mut response = Response::new(Body::from_stream(ReaderStream::new(file)));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&model.content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition).map_err(ApiError::internal)?,
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&model.size_bytes.to_string()).map_err(ApiError::internal)?,
    );
    Ok(response)
}

async fn delete(
    State(state): State<AppState>,
    user: Required<FileWrite>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    authorized_file(&state, &user, &id).await?;
    service::file::delete(&state.db, &state.config.upload_dir, &id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ApiResponse::<()>::message("file deleted")))
}

async fn authorized_file<P>(
    state: &AppState,
    user: &Required<P>,
    id: &str,
) -> Result<crate::model::file::Model, ApiError> {
    if service::file::path_for(&state.config.upload_dir, id).is_none() {
        return Err(ApiError::not_found("file not found"));
    }
    let model = service::file::find(&state.db, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("file not found"))?;
    if model.owner_id != user.id && !user.has_permission("user:manage") {
        return Err(ApiError::forbidden("cannot access another user's file"));
    }
    Ok(model)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(upload))
        .route("/{id}", get(metadata).delete(delete))
        .route("/{id}/download", get(download))
}
