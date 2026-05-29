use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};

use crate::entity::group;

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupModel {
    pub id: i64,
    pub conversation_id: i64,
    pub name: String,
    pub owner_id: i64,
    pub avatar: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn create_group(
    db: &DatabaseConnection,
    conversation_id: i64,
    name: String,
    owner_id: i64,
    avatar: Option<String>,
) -> Result<group::Model, sea_orm::DbErr> {
    group::ActiveModel {
        conversation_id: Set(conversation_id),
        name: Set(name),
        owner_id: Set(owner_id),
        avatar: Set(avatar),
        ..Default::default()
    }
    .insert(db)
    .await
}

pub async fn find_by_conversation_id(
    db: &DatabaseConnection,
    conversation_id: i64,
) -> Result<Option<group::Model>, sea_orm::DbErr> {
    group::Entity::find()
        .filter(group::Column::ConversationId.eq(conversation_id))
        .one(db)
        .await
}

pub async fn set_owner(
    db: &DatabaseConnection,
    model: group::Model,
    new_owner_id: i64,
) -> Result<group::Model, sea_orm::DbErr> {
    let mut am: group::ActiveModel = model.into();
    am.owner_id = Set(new_owner_id);
    am.update(db).await
}
