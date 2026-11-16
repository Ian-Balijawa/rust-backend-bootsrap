//! PostgreSQL persistence for file metadata. Bytes are managed by the file service.

use crate::model::file;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
    sea_query::Expr,
};

pub async fn insert(
    db: &DatabaseConnection,
    id: String,
    owner_id: i32,
    filename: &str,
    content_type: &str,
    size_bytes: i64,
) -> Result<file::Model, DbErr> {
    file::ActiveModel {
        id: Set(id),
        owner_id: Set(owner_id),
        filename: Set(filename.to_string()),
        content_type: Set(content_type.to_string()),
        size_bytes: Set(size_bytes),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        deleted_at: Set(None),
    }
    .insert(db)
    .await
}

pub async fn find_for_owner(
    db: &DatabaseConnection,
    id: &str,
    owner_id: i32,
) -> Result<Option<file::Model>, DbErr> {
    file::Entity::find_by_id(id)
        .filter(file::Column::OwnerId.eq(owner_id))
        .filter(file::Column::DeletedAt.is_null())
        .one(db)
        .await
}

pub async fn delete(db: &DatabaseConnection, id: &str) -> Result<(), DbErr> {
    let now = Utc::now();
    file::Entity::update_many()
        .col_expr(file::Column::DeletedAt, Expr::value(now))
        .col_expr(file::Column::UpdatedAt, Expr::value(now))
        .filter(file::Column::Id.eq(id))
        .filter(file::Column::DeletedAt.is_null())
        .exec(db)
        .await?;
    Ok(())
}
