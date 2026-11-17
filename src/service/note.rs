use crate::{model::note::Note, repository};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum NoteError {
    #[error("note not found")]
    NotFound,
    #[error("title must be between 1 and 200 characters")]
    InvalidTitle,
    #[error("body must be at most 10000 characters")]
    InvalidBody,
    #[error("invalid pagination, search, or sort parameter")]
    InvalidQuery,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

fn title(value: &str) -> Result<&str, NoteError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 200 {
        Err(NoteError::InvalidTitle)
    } else {
        Ok(value)
    }
}

fn body(value: &str) -> Result<&str, NoteError> {
    if value.len() > 10_000 {
        Err(NoteError::InvalidBody)
    } else {
        Ok(value)
    }
}

pub async fn create(
    pool: &PgPool,
    owner_id: i32,
    note_title: &str,
    note_body: &str,
) -> Result<Note, NoteError> {
    Ok(repository::note::insert(
        pool,
        Uuid::now_v7(),
        owner_id,
        title(note_title)?,
        body(note_body)?,
    )
    .await?)
}

pub async fn get(pool: &PgPool, owner_id: i32, id: Uuid) -> Result<Note, NoteError> {
    repository::note::find_by_id(pool, owner_id, id)
        .await?
        .ok_or(NoteError::NotFound)
}

pub async fn list(
    pool: &PgPool,
    owner_id: i32,
    page: u64,
    per_page: u64,
    search: Option<&str>,
    sort: Option<&str>,
) -> Result<(Vec<Note>, u64), NoteError> {
    if page == 0 || !(1..=100).contains(&per_page) || search.is_some_and(|s| s.len() > 100) {
        return Err(NoteError::InvalidQuery);
    }
    let sort = match sort.unwrap_or("created_desc") {
        "created_asc" => "created_asc",
        "created_desc" => "created_desc",
        "title_asc" => "title_asc",
        "title_desc" => "title_desc",
        _ => return Err(NoteError::InvalidQuery),
    };
    let offset = page
        .checked_sub(1)
        .and_then(|value| value.checked_mul(per_page))
        .and_then(|value| i64::try_from(value).ok())
        .ok_or(NoteError::InvalidQuery)?;
    let search = search.map(str::trim).filter(|value| !value.is_empty());
    let (notes, total) =
        repository::note::list(pool, owner_id, search, sort, per_page as i64, offset).await?;
    Ok((
        notes,
        u64::try_from(total).map_err(|_| NoteError::InvalidQuery)?,
    ))
}

pub async fn update(
    pool: &PgPool,
    owner_id: i32,
    id: Uuid,
    note_title: Option<&str>,
    note_body: Option<&str>,
) -> Result<Note, NoteError> {
    let note_title = note_title.map(title).transpose()?;
    let note_body = note_body.map(body).transpose()?;
    repository::note::update(pool, owner_id, id, note_title, note_body)
        .await?
        .ok_or(NoteError::NotFound)
}

pub async fn delete(pool: &PgPool, owner_id: i32, id: Uuid) -> Result<(), NoteError> {
    if repository::note::soft_delete(pool, owner_id, id).await? {
        Ok(())
    } else {
        Err(NoteError::NotFound)
    }
}
