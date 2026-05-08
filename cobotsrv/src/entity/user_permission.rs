use crate::IdType;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "user_permission")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: IdType,
    #[sea_orm(primary_key, auto_increment = false)]
    pub permission_id: IdType,
    /// `true` = explicit allow (overrides RBAC absence), `false` = explicit deny
    /// (overrides RBAC grant). Mirrors `user_authority.allow`.
    pub allow: bool,
    pub create_time: DateTime,
    pub update_time: DateTime,
}

impl Entity {
    /// All explicit overrides for this user — caller resolves precedence vs RBAC.
    pub async fn list_by_user(
        db: &DatabaseConnection,
        user_id: IdType,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .all(db)
            .await
    }
}

impl ActiveModelBehavior for ActiveModel {}
