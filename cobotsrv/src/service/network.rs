use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use dashmap::DashMap;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set,
};
use tracing::{error, info};

use crate::config::IpPoolConfig;
use crate::entity::easytier_network;

/// Shared heartbeat cache to reduce DB write pressure.
pub struct HeartbeatCache {
    inner: DashMap<String, i64>,
    lease_days: u32,
}

impl HeartbeatCache {
    pub fn new(lease_days: u32) -> Self {
        Self {
            inner: DashMap::new(),
            lease_days,
        }
    }

    pub fn touch(&self, robot_sn: &str) {
        let ts = Utc::now().timestamp();
        self.inner.insert(robot_sn.to_string(), ts);
    }

    pub async fn flush(&self, db: &DatabaseConnection) {
        let now = Utc::now().naive_utc();
        let lease = now + chrono::Duration::days(self.lease_days as i64);
        for entry in self.inner.iter() {
            let key = entry.key().clone();
            if let Ok(Some(record)) = easytier_network::Entity::find()
                .filter(easytier_network::Column::RobotSn.eq(&key))
                .one(db)
                .await
            {
                let mut active: easytier_network::ActiveModel = record.into();
                active.last_online_at = Set(Some(now));
                active.lease_expired_at = Set(Some(lease));
                let _ = active.update(db).await;
            }
        }
        self.inner.clear();
    }
}

use std::sync::atomic::{AtomicBool, Ordering};

static HEARTBEAT_INIT: AtomicBool = AtomicBool::new(false);
static HEARTBEAT_CACHE: std::sync::LazyLock<std::sync::Mutex<Option<HeartbeatCache>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// Initialize the heartbeat cache with pool config.
pub fn init_heartbeat_cache(pool: &IpPoolConfig) {
    let mut cache = HEARTBEAT_CACHE.lock().unwrap();
    if HEARTBEAT_INIT.load(Ordering::Relaxed) {
        return;
    }
    *cache = Some(HeartbeatCache::new(pool.lease_days));
    HEARTBEAT_INIT.store(true, Ordering::Relaxed);
}

/// IP allocation service for the easytier P2P network.
pub struct NetworkService;

impl NetworkService {
    /// Main entry point: register a device or return its existing config.
    pub async fn register_or_get_config(
        db: &DatabaseConnection,
        robot_sn: &str,
        pool: &IpPoolConfig,
    ) -> crate::result::Result<easytier_network::Model> {
        let existing = easytier_network::Entity::find()
            .filter(easytier_network::Column::RobotSn.eq(robot_sn))
            .one(db)
            .await?;

        match existing {
            Some(record) if record.virtual_ip.is_some() => {
                let existing_ip = record.virtual_ip.as_deref().unwrap_or("");
                let ip_valid = existing_ip.split('.').count() == 4
                    && existing_ip.parse::<std::net::Ipv4Addr>().is_ok();
                if ip_valid {
                    let now = Utc::now().naive_utc();
                    let lease = now + chrono::Duration::days(pool.lease_days as i64);

                    let mut active: easytier_network::ActiveModel = record.clone().into();
                    active.status = Set("ACTIVE".to_string());
                    active.last_online_at = Set(Some(now));
                    active.lease_expired_at = Set(Some(lease));
                    active.update(db).await?;

                    info!(
                        robot_sn = robot_sn,
                        ip = record.virtual_ip.as_deref(),
                        "easytier device reconnected, returned existing IP"
                    );
                    return Ok(record);
                }
                // IP is malformed — re-allocate immediately.
                tracing::warn!(
                    robot_sn = robot_sn,
                    old_ip = existing_ip,
                    "existing IP is malformed, forcing re-allocation"
                );
                // Re-allocate inline (same logic as the next arm).
                let new_ip = Self::allocate_new_ip(db, pool).await?;
                let now = Utc::now().naive_utc();
                let lease = now + chrono::Duration::days(pool.lease_days as i64);

                let mut active: easytier_network::ActiveModel = record.clone().into();
                active.virtual_ip = Set(Some(new_ip.clone()));
                active.status = Set("ACTIVE".to_string());
                active.last_online_at = Set(Some(now));
                active.lease_expired_at = Set(Some(lease));
                let updated = active.update(db).await?;

                info!(
                    robot_sn = robot_sn,
                    ip = new_ip,
                    "easytier device re-allocated IP (malformed old IP)"
                );
                Ok(updated)
            }
            Some(record) => {
                let new_ip = Self::allocate_new_ip(db, pool).await?;
                let now = Utc::now().naive_utc();
                let lease = now + chrono::Duration::days(pool.lease_days as i64);

                let mut active: easytier_network::ActiveModel = record.clone().into();
                active.virtual_ip = Set(Some(new_ip.clone()));
                active.status = Set("ACTIVE".to_string());
                active.last_online_at = Set(Some(now));
                active.lease_expired_at = Set(Some(lease));
                let updated = active.update(db).await?;

                info!(
                    robot_sn = robot_sn,
                    ip = new_ip,
                    "easytier device re-allocated IP after reclaim"
                );
                Ok(updated)
            }
            None => {
                let new_ip = Self::allocate_new_ip(db, pool).await?;
                let now = Utc::now().naive_utc();
                let lease = now + chrono::Duration::days(pool.lease_days as i64);

                let model = easytier_network::ActiveModel {
                    robot_sn: Set(robot_sn.to_string()),
                    virtual_ip: Set(Some(new_ip.clone())),
                    status: Set("ACTIVE".to_string()),
                    allocated_at: Set(now),
                    last_online_at: Set(Some(now)),
                    lease_expired_at: Set(Some(lease)),
                    ..Default::default()
                };

                let mut retries = 3;
                let inserted = loop {
                    match model.clone().insert(db).await {
                        Ok(record) => break record,
                        Err(sea_orm::DbErr::Query(e)) if e.to_string().contains("UNIQUE") => {
                            retries -= 1;
                            if retries == 0 {
                                return Err(crate::error::AppError::Throw(
                                    "IP allocation failed after 3 retries".into(),
                                ));
                            }
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                        Err(e) => return Err(e.into()),
                    }
                };

                info!(
                    robot_sn = robot_sn,
                    ip = new_ip,
                    "easytier new device allocated IP"
                );
                Ok(inserted)
            }
        }
    }

    /// Find the first available IP in the pool using HashSet for O(1) lookup.
    async fn allocate_new_ip(
        db: &DatabaseConnection,
        pool: &IpPoolConfig,
    ) -> crate::result::Result<String> {
        let used: Vec<String> = easytier_network::Entity::find()
            .select_only()
            .column(easytier_network::Column::VirtualIp)
            .filter(easytier_network::Column::VirtualIp.is_not_null())
            .into_tuple::<String>()
            .all(db)
            .await?
            .into_iter()
            .collect();

        let used_set: HashSet<&str> = used.iter().map(|s| s.as_str()).collect();

        for i in pool.start..=pool.end {
            let ip = format!("{}.{}", pool.prefix, i);
            if !used_set.contains(ip.as_str()) {
                return Ok(ip);
            }
        }

        Err(crate::error::AppError::Throw(
            "IP pool exhausted (10.100.0.10-250)".into(),
        ))
    }

    /// Record a heartbeat — lightweight in-memory touch.
    pub fn heartbeat(robot_sn: &str) {
        let guard = HEARTBEAT_CACHE.lock().unwrap();
        if let Some(ref cache) = *guard {
            cache.touch(robot_sn);
        }
    }

    /// Flush cached heartbeats to database.
    pub async fn flush_heartbeats(db: &DatabaseConnection) {
        // Clone inner map entries under a quick lock, then flush without holding the lock.
        let snapshot: Vec<(String, i64)> = {
            let guard = HEARTBEAT_CACHE.lock().unwrap();
            match &*guard {
                Some(cache) => cache
                    .inner
                    .iter()
                    .map(|e| (e.key().clone(), *e.value()))
                    .collect(),
                None => return,
            }
        };
        // Clear after snapshot
        {
            let guard = HEARTBEAT_CACHE.lock().unwrap();
            if let Some(ref cache) = *guard {
                cache.inner.clear();
            }
        }

        let now = Utc::now().naive_utc();
        // lease_days from the first spawn should be used; for snapshot flush we use default 30
        let lease = now + chrono::Duration::days(30);
        for (robot_sn, _ts) in snapshot {
            if let Ok(Some(record)) = easytier_network::Entity::find()
                .filter(easytier_network::Column::RobotSn.eq(&robot_sn))
                .one(db)
                .await
            {
                let mut active: easytier_network::ActiveModel = record.into();
                active.last_online_at = Set(Some(now));
                active.lease_expired_at = Set(Some(lease));
                let _ = active.update(db).await;
            }
        }
    }

    /// Reclaim expired IPs (devices inactive for 30 days).
    pub async fn reclaim_expired(db: &DatabaseConnection) -> crate::result::Result<u64> {
        let now = Utc::now().naive_utc();
        let expired = easytier_network::Entity::find()
            .filter(easytier_network::Column::Status.eq("ACTIVE"))
            .filter(easytier_network::Column::LeaseExpiredAt.lt(now))
            .all(db)
            .await?;

        let count = expired.len() as u64;
        for record in expired {
            let mut active: easytier_network::ActiveModel = record.into();
            active.virtual_ip = Set(None);
            active.status = Set("RECLAIMED".to_string());
            let _ = active.update(db).await;
        }

        if count > 0 {
            info!(count = count, "reclaimed expired easytier IP leases");
        }
        Ok(count)
    }

    /// Manually scrap a device (admin action).
    pub async fn scrap_device(
        db: &DatabaseConnection,
        robot_sn: &str,
    ) -> crate::result::Result<()> {
        if let Some(record) = easytier_network::Entity::find()
            .filter(easytier_network::Column::RobotSn.eq(robot_sn))
            .one(db)
            .await?
        {
            let mut active: easytier_network::ActiveModel = record.into();
            active.virtual_ip = Set(None);
            active.status = Set("SCRAPPED".to_string());
            active.update(db).await?;
        }
        info!(robot_sn = robot_sn, "easytier device scrapped");
        Ok(())
    }

    /// List all managed devices.
    pub async fn list_devices(
        db: &DatabaseConnection,
    ) -> crate::result::Result<Vec<easytier_network::Model>> {
        easytier_network::Entity::find()
            .all(db)
            .await
            .map_err(Into::into)
    }

    /// Spawn periodic reclaim + heartbeat flush tasks.
    pub fn spawn_reclaim_task(
        db: Arc<crate::service::database::DatabaseService>,
        pool: IpPoolConfig,
    ) {
        // Initialize heartbeat cache with configured lease duration.
        init_heartbeat_cache(&pool);

        let db_c = db.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(86400));
            loop {
                interval.tick().await;
                if let Err(e) = Self::reclaim_expired(&db_c.connection).await {
                    error!("easytier IP reclaim failed: {:?}", e);
                }
            }
        });

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));
            loop {
                interval.tick().await;
                Self::flush_heartbeats(&db.connection).await;
            }
        });
    }
}
