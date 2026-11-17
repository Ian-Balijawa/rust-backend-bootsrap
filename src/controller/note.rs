use crate::{
    AppState,
    http::{
        response::{ApiError, ApiResponse},
        validated_json::ValidatedJson,
    },
    model::note::Note,
    service::{self, auth::AuthenticatedUser},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize, Validate, ToSchema)]
pub(crate) struct CreateNoteRequest {
    #[validate(length(min = 1, max = 200))]
    title: String,
    #[validate(length(max = 10000))]
    body: String,
}

#[derive(Deserialize, Validate, ToSchema)]
pub(crate) struct UpdateNoteRequest {
    #[validate(length(min = 1, max = 200))]
    title: Option<String>,
    #[validate(length(max = 10000))]
    body: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct ListNotesQuery {
    #[serde(default = "default_page")]
    page: u64,
    #[serde(default = "default_per_page")]
    per_page: u64,
    search: Option<String>,
    sort: Option<String>,
}

fn default_page() -> u64 {
    1
}
fn default_per_page() -> u64 {
    20
}

#[derive(Serialize)]
pub(crate) struct NoteResponse {
    id: Uuid,
    owner_id: i32,
    title: String,
    body: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<Note> for NoteResponse {
    fn from(note: Note) -> Self {
        Self {
            id: note.id,
            owner_id: note.owner_id,
            title: note.title,
            body: note.body,
            created_at: note.created_at,
            updated_at: note.updated_at,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct PageMeta {
    page: u64,
    per_page: u64,
    total: u64,
    total_pages: u64,
}

#[derive(Serialize)]
pub(crate) struct NotePage {
    items: Vec<NoteResponse>,
    meta: PageMeta,
}

fn note_error(error: service::note::NoteError) -> ApiError {
    match error {
        service::note::NoteError::NotFound => ApiError::not_found("note not found"),
        service::note::NoteError::Database(error) => ApiError::internal(error),
        _ => ApiError::bad_request(error.to_string()),
    }
}

#[utoipa::path(post, path = "/api/v1/notes", tag = "note", request_body = CreateNoteRequest, security(("bearerAuth" = [])), responses((status = 201, description = "Create note")))]
pub(crate) async fn create(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    ValidatedJson(body): ValidatedJson<CreateNoteRequest>,
) -> Result<(StatusCode, Json<ApiResponse<NoteResponse>>), ApiError> {
    let note = service::note::create(
        state.db.get_postgres_connection_pool(),
        auth.id,
        &body.title,
        &body.body,
    )
    .await
    .map_err(note_error)?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(note.into()))))
}

#[utoipa::path(get, path = "/api/v1/notes", tag = "note", security(("bearerAuth" = [])), responses((status = 200, description = "List notes")))]
pub(crate) async fn list(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Query(query): Query<ListNotesQuery>,
) -> Result<Json<ApiResponse<NotePage>>, ApiError> {
    let (notes, total) = service::note::list(
        state.db.get_postgres_connection_pool(),
        auth.id,
        query.page,
        query.per_page,
        query.search.as_deref(),
        query.sort.as_deref(),
    )
    .await
    .map_err(note_error)?;
    Ok(Json(ApiResponse::ok(NotePage {
        items: notes.into_iter().map(NoteResponse::from).collect(),
        meta: PageMeta {
            page: query.page,
            per_page: query.per_page,
            total,
            total_pages: total.div_ceil(query.per_page),
        },
    })))
}

#[utoipa::path(get, path = "/api/v1/notes/{id}", tag = "note", security(("bearerAuth" = [])), responses((status = 200, description = "Get note")))]
pub(crate) async fn get(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<NoteResponse>>, ApiError> {
    let note = service::note::get(state.db.get_postgres_connection_pool(), auth.id, id)
        .await
        .map_err(note_error)?;
    Ok(Json(ApiResponse::ok(note.into())))
}

#[utoipa::path(patch, path = "/api/v1/notes/{id}", tag = "note", request_body = UpdateNoteRequest, security(("bearerAuth" = [])), responses((status = 200, description = "Update note")))]
pub(crate) async fn update(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
    ValidatedJson(body): ValidatedJson<UpdateNoteRequest>,
) -> Result<Json<ApiResponse<NoteResponse>>, ApiError> {
    let note = service::note::update(
        state.db.get_postgres_connection_pool(),
        auth.id,
        id,
        body.title.as_deref(),
        body.body.as_deref(),
    )
    .await
    .map_err(note_error)?;
    Ok(Json(ApiResponse::ok(note.into())))
}

#[utoipa::path(delete, path = "/api/v1/notes/{id}", tag = "note", security(("bearerAuth" = [])), responses((status = 204, description = "Delete note")))]
pub(crate) async fn delete(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    service::note::delete(state.db.get_postgres_connection_pool(), auth.id, id)
        .await
        .map_err(note_error)?;
    Ok(StatusCode::NO_CONTENT)
}
