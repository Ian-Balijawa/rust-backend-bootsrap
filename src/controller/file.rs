use crate::{
    AppState,
    http::{
        auth::{FileRead, FileWrite, Required},
        response::{ApiError, ApiResponse},
    },
    service,
};
use axum::{
    Json,
    body::Body,
    extract::{Path, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Serialize;
use tokio_util::io::ReaderStream;

#[derive(Serialize)]
pub(crate) struct FileResponse {
    id: String,
    filename: String,
    content_type: String,
    size_bytes: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<crate::model::file::Model> for FileResponse {
    fn from(file: crate::model::file::Model) -> Self {
        Self {
            id: file.id,
            filename: file.filename,
            content_type: file.content_type,
            size_bytes: file.size_bytes,
            created_at: file.created_at,
            updated_at: file.updated_at,
        }
    }
}

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

#[utoipa::path(post, path = "/api/v1/files", tag = "file", security(("bearerAuth" = [])), responses((status = 201, description = "Upload file")))]
pub(crate) async fn upload(
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
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::ok(FileResponse::from(model))),
    ))
}

#[utoipa::path(get, path = "/api/v1/files/{id}", tag = "file", security(("bearerAuth" = [])), responses((status = 200, description = "Get file metadata")))]
pub(crate) async fn metadata(
    State(state): State<AppState>,
    user: Required<FileRead>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let model = authorized_file(&state, &user, &id).await?;
    Ok(Json(ApiResponse::ok(FileResponse::from(model))))
}

#[utoipa::path(get, path = "/api/v1/files/{id}/download", tag = "file", security(("bearerAuth" = [])), responses((status = 200, description = "Download file")))]
pub(crate) async fn download(
    State(state): State<AppState>,
    user: Required<FileRead>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let model = authorized_file(&state, &user, &id).await?;
    let file = service::file::open(&state.config.upload_dir, &model.id)
        .await
        .map_err(|error| {
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

#[utoipa::path(delete, path = "/api/v1/files/{id}", tag = "file", security(("bearerAuth" = [])), responses((status = 200, description = "Delete file")))]
pub(crate) async fn delete(
    State(state): State<AppState>,
    user: Required<FileWrite>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    authorized_file(&state, &user, &id).await?;
    service::file::delete(&state.db, &id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ApiResponse::<()>::message("file deleted")))
}

async fn authorized_file<P>(
    state: &AppState,
    user: &Required<P>,
    id: &str,
) -> Result<crate::model::file::Model, ApiError> {
    let model = service::file::find_for_owner(&state.db, &state.config.upload_dir, user.id, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("file not found"))?;
    Ok(model)
}
