use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "password_reset_tokens")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub token_hash: String,
    pub user_id: i32,
    pub expires_at: DateTimeUtc,
    pub used_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id",
        on_delete = "Cascade"
    )]
    User,
}

impl ActiveModelBehavior for ActiveModel {}
