use chrono::{NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, QueryOrder, Set,
};

use crate::entity::device;

pub struct DeviceService;

#[derive(Debug)]
pub struct UpsertDevice<'a> {
    pub cobotlet_alias: &'a str,
    pub device_id: &'a str,
    pub service_url: String,
    pub capabilities: serde_json::Value,
    pub online: bool,
    pub last_heartbeat: NaiveDateTime,
    pub registered_at: NaiveDateTime,
}

impl DeviceService {
    pub async fn upsert(
        db: &DatabaseConnection,
        input: UpsertDevice<'_>,
    ) -> Result<device::Model, DbErr> {
        let now = Utc::now().naive_utc();
        match Self::find(db, input.cobotlet_alias, input.device_id).await? {
            Some(existing) => {
                let mut am: device::ActiveModel = existing.into();
                am.service_url = Set(input.service_url);
                am.capabilities = Set(input.capabilities);
                am.online = Set(input.online);
                am.last_heartbeat = Set(input.last_heartbeat);
                am.registered_at = Set(input.registered_at);
                am.update_time = Set(now);
                am.update(db).await
            }
            None => {
                let am = device::ActiveModel {
                    id: ActiveValue::NotSet,
                    cobotlet_alias: Set(input.cobotlet_alias.to_string()),
                    device_id: Set(input.device_id.to_string()),
                    service_url: Set(input.service_url),
                    capabilities: Set(input.capabilities),
                    online: Set(input.online),
                    last_heartbeat: Set(input.last_heartbeat),
                    registered_at: Set(input.registered_at),
                    update_time: Set(now),
                };
                am.insert(db).await
            }
        }
    }

    pub async fn mark_offline(
        db: &DatabaseConnection,
        cobotlet_alias: &str,
        device_id: &str,
    ) -> Result<Option<device::Model>, DbErr> {
        let Some(existing) = Self::find(db, cobotlet_alias, device_id).await? else {
            return Ok(None);
        };
        let mut am: device::ActiveModel = existing.into();
        am.online = Set(false);
        am.update_time = Set(Utc::now().naive_utc());
        Ok(Some(am.update(db).await?))
    }

    pub async fn update_heartbeat(
        db: &DatabaseConnection,
        cobotlet_alias: &str,
        device_id: &str,
        ts: NaiveDateTime,
    ) -> Result<Option<device::Model>, DbErr> {
        let Some(existing) = Self::find(db, cobotlet_alias, device_id).await? else {
            return Ok(None);
        };
        let mut am: device::ActiveModel = existing.into();
        am.last_heartbeat = Set(ts);
        am.online = Set(true);
        am.update_time = Set(Utc::now().naive_utc());
        Ok(Some(am.update(db).await?))
    }

    pub async fn find(
        db: &DatabaseConnection,
        cobotlet_alias: &str,
        device_id: &str,
    ) -> Result<Option<device::Model>, DbErr> {
        device::Entity::find()
            .filter(device::Column::CobotletAlias.eq(cobotlet_alias))
            .filter(device::Column::DeviceId.eq(device_id))
            .one(db)
            .await
    }

    pub async fn list(db: &DatabaseConnection) -> Result<Vec<device::Model>, DbErr> {
        device::Entity::find()
            .order_by_desc(device::Column::UpdateTime)
            .all(db)
            .await
    }

    pub async fn list_by_alias(
        db: &DatabaseConnection,
        cobotlet_alias: &str,
    ) -> Result<Vec<device::Model>, DbErr> {
        device::Entity::find()
            .filter(device::Column::CobotletAlias.eq(cobotlet_alias))
            .order_by_desc(device::Column::UpdateTime)
            .all(db)
            .await
    }
}
