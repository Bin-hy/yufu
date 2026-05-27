use chrono::{NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait,
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

    /// List devices with optional server-side filters for alias and online status.
    /// The `capability` filter is applied in memory after the DB query because
    /// the capabilities column is unindexed JSON.
    pub async fn list_filtered(
        db: &DatabaseConnection,
        alias: Option<&str>,
        online: Option<bool>,
        capability: Option<&str>,
    ) -> Result<Vec<device::Model>, DbErr> {
        let mut cond = Condition::all();
        if let Some(a) = alias {
            cond = cond.add(device::Column::CobotletAlias.eq(a));
        }
        if let Some(o) = online {
            cond = cond.add(device::Column::Online.eq(o));
        }
        let mut rows = device::Entity::find()
            .filter(cond)
            .order_by_desc(device::Column::UpdateTime)
            .all(db)
            .await?;
        if let Some(cap) = capability {
            rows.retain(|d| Self::matches_capability(&d.capabilities, cap));
        }
        Ok(rows)
    }

    /// Pure in-memory capability filter used by `list_filtered`. Extracted so it
    /// can be unit-tested without a database connection.
    pub(crate) fn matches_capability(capabilities: &serde_json::Value, name: &str) -> bool {
        capabilities
            .as_array()
            .map(|arr| {
                arr.iter()
                    .any(|c| c.get("name").and_then(|n| n.as_str()) == Some(name))
            })
            .unwrap_or(false)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn matches_capability_found() {
        let caps = json!([
            { "name": "move", "method": "POST", "path": "/move" },
            { "name": "grip", "method": "POST", "path": "/grip" }
        ]);
        assert!(DeviceService::matches_capability(&caps, "move"));
        assert!(DeviceService::matches_capability(&caps, "grip"));
    }

    #[test]
    fn matches_capability_not_found() {
        let caps = json!([{ "name": "move", "method": "POST", "path": "/move" }]);
        assert!(!DeviceService::matches_capability(&caps, "camera"));
    }

    #[test]
    fn matches_capability_empty_array() {
        let caps = json!([]);
        assert!(!DeviceService::matches_capability(&caps, "move"));
    }

    #[test]
    fn matches_capability_non_array_json() {
        // Malformed capabilities column should not panic.
        let caps = json!(null);
        assert!(!DeviceService::matches_capability(&caps, "move"));
        let caps = json!("not-an-array");
        assert!(!DeviceService::matches_capability(&caps, "move"));
    }

    #[test]
    fn matches_capability_partial_name_does_not_match() {
        let caps = json!([{ "name": "movement", "method": "POST", "path": "/movement" }]);
        assert!(!DeviceService::matches_capability(&caps, "move"));
    }
}
