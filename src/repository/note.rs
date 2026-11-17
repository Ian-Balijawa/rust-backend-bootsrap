use crate::model::note::Note;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn insert(
    pool: &PgPool,
    id: Uuid,
    owner_id: i32,
    title: &str,
    body: &str,
) -> Result<Note, sqlx::Error> {
    sqlx::query_as!(
        Note,
        "INSERT INTO notes (id, owner_id, title, body) VALUES ($1, $2, $3, $4)
         RETURNING id, owner_id, title, body, created_at, updated_at",
        id,
        owner_id,
        title,
        body,
    )
    .fetch_one(pool)
    .await
}

pub async fn find_by_id(
    pool: &PgPool,
    owner_id: i32,
    id: Uuid,
) -> Result<Option<Note>, sqlx::Error> {
    sqlx::query_as!(
        Note,
        "SELECT id, owner_id, title, body, created_at, updated_at
         FROM notes WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL",
        id,
        owner_id,
    )
    .fetch_optional(pool)
    .await
}

pub async fn list(
    pool: &PgPool,
    owner_id: i32,
    search: Option<&str>,
    sort: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Note>, i64), sqlx::Error> {
    let total = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM notes
         WHERE owner_id = $1 AND deleted_at IS NULL
         AND ($2::text IS NULL OR title ILIKE '%' || $2 || '%')",
        owner_id,
        search,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(0);
    let rows = sqlx::query_as!(
        Note,
        "SELECT id, owner_id, title, body, created_at, updated_at
         FROM notes
         WHERE owner_id = $1 AND deleted_at IS NULL
         AND ($2::text IS NULL OR title ILIKE '%' || $2 || '%')
         ORDER BY
           CASE WHEN $3::text = 'created_asc' THEN created_at END ASC,
           CASE WHEN $3::text = 'created_desc' THEN created_at END DESC,
           CASE WHEN $3::text = 'title_asc' THEN title END ASC,
           CASE WHEN $3::text = 'title_desc' THEN title END DESC,
           id ASC
         LIMIT $4 OFFSET $5",
        owner_id,
        search,
        sort,
        limit,
        offset,
    )
    .fetch_all(pool)
    .await?;
    Ok((rows, total))
}

pub async fn update(
    pool: &PgPool,
    owner_id: i32,
    id: Uuid,
    title: Option<&str>,
    body: Option<&str>,
) -> Result<Option<Note>, sqlx::Error> {
    sqlx::query_as!(
        Note,
        "UPDATE notes SET title = COALESCE($3, title), body = COALESCE($4, body), updated_at = now()
         WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL
         RETURNING id, owner_id, title, body, created_at, updated_at",
        id,
        owner_id,
        title,
        body,
    )
    .fetch_optional(pool)
    .await
}

pub async fn soft_delete(pool: &PgPool, owner_id: i32, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        "UPDATE notes SET deleted_at = now(), updated_at = now()
         WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL",
        id,
        owner_id,
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}
