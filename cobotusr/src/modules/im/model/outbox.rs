use serde::{Deserialize, Serialize};
use serde_json::Value;

use chrono::{Duration, FixedOffset, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, QueryFilter, QueryOrder, QuerySelect, Set,
};

use crate::modules::im::model::enums::OutboxStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxPushPayload {
    pub targets: Vec<i64>,
    pub envelope: Value,
}

#[derive(Debug, Clone)]
pub struct OutboxTask {
    pub id: i64,
    pub message_id: i64,
    pub conversation_id: i64,
    pub payload: Value,
    pub retry_count: i32,
}

fn now_fixed() -> chrono::DateTime<FixedOffset> {
    Utc::now().fixed_offset()
}

fn to_task(model: Model) -> OutboxTask {
    OutboxTask {
        id: model.id,
        message_id: model.message_id,
        conversation_id: model.conversation_id,
        payload: model.payload,
        retry_count: model.retry_count,
    }
}

pub async fn insert_pending(
    db: &DatabaseConnection,
    message_id: i64,
    conversation_id: i64,
    payload: Value,
) -> Result<(), sea_orm::DbErr> {
    Entity::insert(ActiveModel {
        message_id: Set(message_id),
        conversation_id: Set(conversation_id),
        payload: Set(payload),
        status: Set(OutboxStatus::Pending),
        retry_count: Set(0),
        next_retry_at: Set(now_fixed()),
        last_error: Set(None),
        sent_at: Set(None),
        ..Default::default()
    })
    .on_conflict(
        OnConflict::column(Column::MessageId)
            .do_nothing()
            .to_owned(),
    )
    .exec(db)
    .await?;

    Ok(())
}

pub async fn list_due_tasks(
    db: &DatabaseConnection,
    limit: u64,
) -> Result<Vec<OutboxTask>, sea_orm::DbErr> {
    let rows = Entity::find()
        .filter(Column::Status.is_in([OutboxStatus::Pending, OutboxStatus::Retrying]))
        .filter(Column::NextRetryAt.lte(now_fixed()))
        .order_by_asc(Column::Id)
        .limit(limit)
        .all(db)
        .await?;

    Ok(rows.into_iter().map(to_task).collect())
}

pub async fn list_pending_retry_tasks(
    db: &DatabaseConnection,
    limit: u64,
) -> Result<Vec<OutboxTask>, sea_orm::DbErr> {
    let rows = Entity::find()
        .filter(Column::Status.is_in([OutboxStatus::Pending, OutboxStatus::Retrying]))
        .order_by_asc(Column::Id)
        .limit(limit)
        .all(db)
        .await?;

    Ok(rows.into_iter().map(to_task).collect())
}

pub async fn mark_sent(db: &DatabaseConnection, outbox_id: i64) -> Result<(), sea_orm::DbErr> {
    let Some(row) = Entity::find_by_id(outbox_id).one(db).await? else {
        return Ok(());
    };

    let now = now_fixed();
    let mut am: ActiveModel = row.into();
    am.status = Set(OutboxStatus::Sent);
    am.sent_at = Set(Some(now));
    am.last_error = Set(None);
    am.updated_at = Set(now);
    am.update(db).await?;
    Ok(())
}

pub async fn mark_retry(
    db: &DatabaseConnection,
    outbox_id: i64,
    current_retry_count: i32,
    max_retry: i32,
    retry_backoff_secs: i32,
    last_error: &str,
) -> Result<(), sea_orm::DbErr> {
    let Some(row) = Entity::find_by_id(outbox_id).one(db).await? else {
        return Ok(());
    };

    let next_retry_count = current_retry_count.saturating_add(1);
    let status = if next_retry_count >= max_retry {
        OutboxStatus::Failed
    } else {
        OutboxStatus::Retrying
    };
    let now = now_fixed();
    let next_retry_at = (Utc::now() + Duration::seconds(retry_backoff_secs as i64)).fixed_offset();

    let mut am: ActiveModel = row.into();
    am.status = Set(status);
    am.retry_count = Set(next_retry_count);
    am.next_retry_at = Set(next_retry_at);
    am.last_error = Set(Some(last_error.to_string()));
    am.updated_at = Set(now);
    am.update(db).await?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "message_outbox")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub message_id: i64,
    pub conversation_id: i64,
    pub payload: Value,
    pub status: OutboxStatus,
    pub retry_count: i32,
    pub next_retry_at: DateTimeWithTimeZone,
    pub last_error: Option<String>,
    pub sent_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
