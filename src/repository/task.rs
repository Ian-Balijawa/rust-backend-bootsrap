//! Owner-scoped PostgreSQL persistence for tasks.

use crate::model::task;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set, sea_query::Expr,
};

pub async fn add_task(
    db: &DatabaseConnection,
    owner_id: i32,
    action: &str,
    schedule_time: DateTime<Utc>,
) -> Result<task::Model, DbErr> {
    let now = Utc::now();
    task::ActiveModel {
        user_id: Set(owner_id),
        action: Set(action.to_string()),
        schedule_time: Set(schedule_time),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
        ..Default::default()
    }
    .insert(db)
    .await
}

pub async fn list_for_owner(
    db: &DatabaseConnection,
    owner_id: i32,
    page: u64,
    per_page: u64,
) -> Result<(Vec<task::Model>, u64), DbErr> {
    let query = task::Entity::find()
        .filter(task::Column::UserId.eq(owner_id))
        .filter(task::Column::DeletedAt.is_null());
    let total = query.clone().count(db).await?;
    let rows = query
        .order_by_asc(task::Column::ScheduleTime)
        .order_by_asc(task::Column::Id)
        .paginate(db, per_page)
        .fetch_page(page.saturating_sub(1))
        .await?;
    Ok((rows, total))
}

pub async fn find_for_owner(
    db: &DatabaseConnection,
    owner_id: i32,
    id: i32,
) -> Result<Option<task::Model>, DbErr> {
    task::Entity::find_by_id(id)
        .filter(task::Column::UserId.eq(owner_id))
        .filter(task::Column::DeletedAt.is_null())
        .one(db)
        .await
}

pub async fn update_for_owner(
    db: &DatabaseConnection,
    owner_id: i32,
    id: i32,
    action: Option<String>,
    schedule_time: Option<DateTime<Utc>>,
) -> Result<Option<task::Model>, DbErr> {
    let mut query = task::Entity::update_many()
        .col_expr(task::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(task::Column::Id.eq(id))
        .filter(task::Column::UserId.eq(owner_id))
        .filter(task::Column::DeletedAt.is_null());
    if let Some(action) = action {
        query = query.col_expr(task::Column::Action, Expr::value(action));
    }
    if let Some(schedule_time) = schedule_time {
        query = query.col_expr(task::Column::ScheduleTime, Expr::value(schedule_time));
    }
    if query.exec(db).await?.rows_affected == 0 {
        return Ok(None);
    }
    find_for_owner(db, owner_id, id).await
}

pub async fn soft_delete_for_owner(
    db: &DatabaseConnection,
    owner_id: i32,
    id: i32,
) -> Result<bool, DbErr> {
    let now = Utc::now();
    let result = task::Entity::update_many()
        .col_expr(task::Column::DeletedAt, Expr::value(now))
        .col_expr(task::Column::UpdatedAt, Expr::value(now))
        .filter(task::Column::Id.eq(id))
        .filter(task::Column::UserId.eq(owner_id))
        .filter(task::Column::DeletedAt.is_null())
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}
