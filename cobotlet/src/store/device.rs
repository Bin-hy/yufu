use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capability {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_method")]
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, alias = "schema")]
    pub input_schema: Option<serde_json::Value>,
}

fn default_method() -> String {
    String::from("POST")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DeviceState {
    #[default]
    Registering,
    Ready,
    Busy {
        task_id: String,
    },
    Error {
        reason: String,
    },
    Maintenance,
    Offline,
}

impl DeviceState {
    pub fn can_transition_to(&self, target: &DeviceState) -> bool {
        matches!(
            (self, target),
            (Self::Registering, Self::Ready)
                | (Self::Registering, Self::Offline { .. })
                | (Self::Ready, Self::Busy { .. })
                | (Self::Ready, Self::Maintenance)
                | (Self::Ready, Self::Offline { .. })
                | (Self::Busy { .. }, Self::Ready)
                | (Self::Busy { .. }, Self::Error { .. })
                | (Self::Busy { .. }, Self::Offline { .. })
                | (Self::Error { .. }, Self::Ready)
                | (Self::Error { .. }, Self::Maintenance)
                | (Self::Error { .. }, Self::Offline { .. })
                | (Self::Maintenance, Self::Ready)
                | (Self::Maintenance, Self::Offline { .. })
                | (Self::Offline { .. }, Self::Ready)
                | (Self::Offline { .. }, Self::Registering)
        )
    }

    pub fn is_available(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Registering => "registering",
            Self::Ready => "ready",
            Self::Busy { .. } => "busy",
            Self::Error { .. } => "error",
            Self::Maintenance => "maintenance",
            Self::Offline => "offline",
        }
    }
}

/// Error produced by [`DeviceStore::transition`].
#[derive(Debug)]
pub enum TransitionError {
    /// The requested transition is rejected by the state machine.
    Invalid { from: String, to: String },
    /// Persistence (redb) failure — the in-memory state was not changed.
    Store(anyhow::Error),
}

impl std::fmt::Display for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransitionError::Invalid { from, to } => {
                write!(f, "invalid transition: {from} → {to}")
            }
            TransitionError::Store(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for TransitionError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub service_url: String,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    pub last_heartbeat: DateTime<Utc>,
    pub registered_at: DateTime<Utc>,
    #[serde(default)]
    pub state: DeviceState,
    #[serde(default)]
    pub control_preset: Option<crate::config::ControlPreset>,
}

const TABLE: TableDefinition<&str, &str> = TableDefinition::new("devices");

pub struct DeviceStore {
    db: Arc<Database>,
    devices: DashMap<String, Device>,
    capability_index: DashMap<String, HashSet<String>>,
}

impl DeviceStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let db = Database::create(path)?;
        Self::new(Arc::new(db))
    }

    pub fn new(db: Arc<Database>) -> Result<Self> {
        let store = Self {
            db,
            devices: DashMap::new(),
            capability_index: DashMap::new(),
        };
        store.load_from_disk()?;
        Ok(store)
    }

    fn load_from_disk(&self) -> Result<()> {
        let read_txn = self.db.begin_read()?;
        let table = match read_txn.open_table(TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        for entry in table.iter()? {
            let (k, v) = entry?;
            let device: Device = serde_json::from_str(v.value())?;
            self.index_capabilities(&device);
            self.devices.insert(k.value().to_string(), device);
        }
        Ok(())
    }

    fn persist(&self, device: &Device) -> Result<()> {
        let json = serde_json::to_string(device)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE)?;
            table.insert(device.id.as_str(), json.as_str())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE)?;
            table.remove(id)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    fn index_capabilities(&self, device: &Device) {
        for cap in &device.capabilities {
            self.capability_index
                .entry(cap.name.clone())
                .or_default()
                .insert(device.id.clone());
        }
    }

    fn unindex_capabilities(&self, device: &Device) {
        for cap in &device.capabilities {
            if let Some(mut set) = self.capability_index.get_mut(&cap.name) {
                set.remove(&device.id);
            }
        }
    }

    pub fn register(&self, device: Device) -> Result<Device> {
        if let Some(old) = self.devices.get(&device.id) {
            let snapshot = old.clone();
            drop(old);
            self.unindex_capabilities(&snapshot);
        }
        self.index_capabilities(&device);
        self.persist(&device)?;
        self.devices.insert(device.id.clone(), device.clone());
        Ok(device)
    }

    /// Returns the updated device and the previous state.
    /// Transitions Registering/Offline/Error → Ready on heartbeat.
    /// Busy/Ready/Maintenance just update last_heartbeat.
    pub fn heartbeat(&self, id: &str) -> Result<Option<(Device, DeviceState)>> {
        let mut entry = match self.devices.get_mut(id) {
            Some(e) => e,
            None => return Ok(None),
        };
        let old_state = entry.state.clone();
        entry.last_heartbeat = Utc::now();
        let transitioned = matches!(
            old_state,
            DeviceState::Registering | DeviceState::Offline | DeviceState::Error { .. }
        );
        if transitioned {
            entry.state = DeviceState::Ready;
        }
        let device = entry.clone();
        drop(entry);
        if transitioned {
            self.persist(&device)?;
        }
        Ok(Some((device, old_state)))
    }

    pub fn transition(&self, id: &str, to: DeviceState) -> Result<Option<Device>, TransitionError> {
        let mut entry = match self.devices.get_mut(id) {
            Some(e) => e,
            None => return Ok(None),
        };
        if !entry.state.can_transition_to(&to) {
            return Err(TransitionError::Invalid {
                from: entry.state.as_str().to_string(),
                to: to.as_str().to_string(),
            });
        }
        entry.state = to;
        let device = entry.clone();
        drop(entry);
        self.persist(&device).map_err(TransitionError::Store)?;
        Ok(Some(device))
    }

    pub fn acquire_for_task(&self, id: &str, task_id: &str) -> Result<Option<Device>> {
        let mut entry = match self.devices.get_mut(id) {
            Some(e) => e,
            None => return Ok(None),
        };
        if !entry.state.is_available() {
            return Ok(None);
        }
        entry.state = DeviceState::Busy {
            task_id: task_id.to_string(),
        };
        let device = entry.clone();
        drop(entry);
        self.persist(&device)?;
        Ok(Some(device))
    }

    pub fn release_from_task(&self, id: &str) -> Result<Option<Device>> {
        let mut entry = match self.devices.get_mut(id) {
            Some(e) => e,
            None => return Ok(None),
        };
        if !matches!(entry.state, DeviceState::Busy { .. }) {
            return Ok(None);
        }
        entry.state = DeviceState::Ready;
        let device = entry.clone();
        drop(entry);
        self.persist(&device)?;
        Ok(Some(device))
    }

    pub fn mark_error(&self, id: &str, reason: String) -> Result<Option<Device>> {
        let mut entry = match self.devices.get_mut(id) {
            Some(e) => e,
            None => return Ok(None),
        };
        if !matches!(entry.state, DeviceState::Busy { .. }) {
            return Ok(None);
        }
        entry.state = DeviceState::Error { reason };
        let device = entry.clone();
        drop(entry);
        self.persist(&device)?;
        Ok(Some(device))
    }

    pub fn list(&self) -> Vec<Device> {
        self.devices.iter().map(|e| e.value().clone()).collect()
    }

    pub fn get(&self, id: &str) -> Option<Device> {
        self.devices.get(id).map(|e| e.clone())
    }

    pub fn remove(&self, id: &str) -> Result<Option<Device>> {
        if let Some((_, device)) = self.devices.remove(id) {
            self.unindex_capabilities(&device);
            self.delete(id)?;
            Ok(Some(device))
        } else {
            Ok(None)
        }
    }

    /// Replace a device's capabilities in place and re-index. Returns the updated
    /// device, or `None` if the device ID is not found.
    pub fn update_capabilities(&self, id: &str, caps: Vec<Capability>) -> Result<Option<Device>> {
        let Some(mut entry) = self.devices.get_mut(id) else {
            return Ok(None);
        };
        self.unindex_capabilities(&entry);
        entry.capabilities = caps;
        let device = entry.clone();
        drop(entry);
        self.index_capabilities(&device);
        self.persist(&device)?;
        Ok(Some(device))
    }

    pub fn find_by_capability(&self, name: &str) -> Vec<Device> {
        let Some(set) = self.capability_index.get(name) else {
            return Vec::new();
        };
        set.iter()
            .filter_map(|id| self.devices.get(id).map(|e| e.clone()))
            .filter(|d| d.state.is_available())
            .collect()
    }

    /// Sweeps devices whose `last_heartbeat` is older than `timeout_secs`,
    /// transitions them to Offline, persists the change, and returns the
    /// snapshots of devices that just transitioned.
    pub fn sweep_offline(&self, timeout_secs: u64) -> Vec<Device> {
        let now = Utc::now();
        let mut newly_offline = Vec::new();
        for mut entry in self.devices.iter_mut() {
            let age = (now - entry.last_heartbeat).num_seconds().max(0) as u64;
            if !matches!(entry.state, DeviceState::Offline | DeviceState::Maintenance)
                && age > timeout_secs
            {
                entry.state = DeviceState::Offline;
                newly_offline.push(entry.clone());
            }
        }
        for d in &newly_offline {
            if let Err(e) = self.persist(d) {
                tracing::warn!(device_id = %d.id, error = %e, "persist offline state failed");
            }
        }
        newly_offline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, caps: &[&str]) -> Device {
        let now = Utc::now();
        Device {
            id: id.to_string(),
            service_url: format!("http://127.0.0.1:9000/{id}"),
            capabilities: caps
                .iter()
                .map(|name| Capability {
                    name: (*name).to_string(),
                    description: None,
                    method: "POST".to_string(),
                    path: format!("/{name}"),
                    tags: vec![],
                    input_schema: None,
                })
                .collect(),
            last_heartbeat: now,
            registered_at: now,
            state: DeviceState::Ready,
            control_preset: None,
        }
    }

    fn store() -> (DeviceStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cobotlet.redb");
        (DeviceStore::open(path).unwrap(), dir)
    }

    #[test]
    fn register_and_lookup() {
        let (s, _g) = store();
        s.register(sample("d1", &["move"])).unwrap();
        assert!(s.get("d1").is_some());
        assert_eq!(s.get("d1").unwrap().state, DeviceState::Ready);
        assert_eq!(s.find_by_capability("move").len(), 1);
        assert_eq!(s.find_by_capability("move")[0].id, "d1");
    }

    #[test]
    fn heartbeat_resurrects_offline() {
        let (s, _g) = store();
        let mut d = sample("d1", &["move"]);
        d.state = DeviceState::Offline;
        d.last_heartbeat = Utc::now() - chrono::Duration::seconds(120);
        s.register(d).unwrap();

        let (after, old_state) = s.heartbeat("d1").unwrap().unwrap();
        assert_eq!(old_state, DeviceState::Offline);
        assert_eq!(after.state, DeviceState::Ready);
        assert_eq!(s.find_by_capability("move").len(), 1);
    }

    #[test]
    fn sweep_marks_stale_offline() {
        let (s, _g) = store();
        let mut d = sample("d1", &["move"]);
        d.state = DeviceState::Ready;
        d.last_heartbeat = Utc::now() - chrono::Duration::seconds(120);
        s.register(d).unwrap();

        let dropped = s.sweep_offline(30);
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].id, "d1");
        assert_eq!(s.get("d1").unwrap().state, DeviceState::Offline);
        assert!(s.find_by_capability("move").is_empty());
        assert!(s.sweep_offline(30).is_empty());
    }

    #[test]
    fn re_register_swaps_capabilities() {
        let (s, _g) = store();
        s.register(sample("d1", &["move"])).unwrap();
        s.register(sample("d1", &["camera"])).unwrap();
        assert!(s.find_by_capability("move").is_empty());
        assert_eq!(s.find_by_capability("camera").len(), 1);
    }

    #[test]
    fn persistence_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cobotlet.redb");
        {
            let s = DeviceStore::open(&path).unwrap();
            s.register(sample("d1", &["move"])).unwrap();
        }
        let s2 = DeviceStore::open(&path).unwrap();
        assert!(s2.get("d1").is_some());
        assert_eq!(s2.get("d1").unwrap().state, DeviceState::Ready);
        assert_eq!(s2.find_by_capability("move").len(), 1);
    }

    #[test]
    fn remove_clears_capability_index() {
        let (s, _g) = store();
        s.register(sample("d1", &["move"])).unwrap();
        s.heartbeat("d1").unwrap();
        s.remove("d1").unwrap();
        assert!(s.find_by_capability("move").is_empty());
        assert!(s.get("d1").is_none());
    }

    #[test]
    fn update_capabilities_replaces_and_reindexes() {
        let (s, _g) = store();
        s.register(sample("d1", &["move", "grip"])).unwrap();
        s.heartbeat("d1").unwrap();

        let new_caps = vec![Capability {
            name: "camera".to_string(),
            description: None,
            method: "POST".to_string(),
            path: "/camera".to_string(),
            tags: vec![],
            input_schema: None,
        }];
        let updated = s.update_capabilities("d1", new_caps).unwrap().unwrap();

        assert_eq!(updated.capabilities.len(), 1);
        assert_eq!(updated.capabilities[0].name, "camera");
        assert_eq!(s.find_by_capability("camera").len(), 1);
        assert!(s.find_by_capability("move").is_empty());
        assert!(s.find_by_capability("grip").is_empty());
    }

    #[test]
    fn update_capabilities_nonexistent_returns_none() {
        let (s, _g) = store();
        let result = s.update_capabilities("no-such-device", vec![]).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn update_capabilities_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cobotlet.redb");
        {
            let s = DeviceStore::open(&path).unwrap();
            s.register(sample("d1", &["move"])).unwrap();
            s.heartbeat("d1").unwrap();
            let caps = vec![Capability {
                name: "grip".to_string(),
                description: None,
                method: "POST".to_string(),
                path: "/grip".to_string(),
                tags: vec![],
                input_schema: None,
            }];
            s.update_capabilities("d1", caps).unwrap();
        }
        let s2 = DeviceStore::open(&path).unwrap();
        assert!(s2.find_by_capability("move").is_empty());
        assert_eq!(s2.find_by_capability("grip").len(), 1);
    }

    #[test]
    fn device_state_valid_transitions() {
        let cases = vec![
            (DeviceState::Registering, DeviceState::Ready),
            (
                DeviceState::Ready,
                DeviceState::Busy {
                    task_id: "t1".into(),
                },
            ),
            (DeviceState::Ready, DeviceState::Maintenance),
            (DeviceState::Ready, DeviceState::Offline),
            (
                DeviceState::Busy {
                    task_id: "t1".into(),
                },
                DeviceState::Ready,
            ),
            (
                DeviceState::Busy {
                    task_id: "t1".into(),
                },
                DeviceState::Error {
                    reason: "500".into(),
                },
            ),
            (
                DeviceState::Busy {
                    task_id: "t1".into(),
                },
                DeviceState::Offline,
            ),
            (
                DeviceState::Error { reason: "x".into() },
                DeviceState::Ready,
            ),
            (
                DeviceState::Error { reason: "x".into() },
                DeviceState::Maintenance,
            ),
            (
                DeviceState::Error { reason: "x".into() },
                DeviceState::Offline,
            ),
            (DeviceState::Maintenance, DeviceState::Ready),
            (DeviceState::Maintenance, DeviceState::Offline),
            (DeviceState::Offline, DeviceState::Ready),
            (DeviceState::Offline, DeviceState::Registering),
        ];
        for (from, to) in cases {
            assert!(
                from.can_transition_to(&to),
                "{from:?} → {to:?} should be valid"
            );
        }
    }

    #[test]
    fn device_state_invalid_transitions() {
        let cases = vec![
            (
                DeviceState::Registering,
                DeviceState::Busy {
                    task_id: "t1".into(),
                },
            ),
            (DeviceState::Registering, DeviceState::Maintenance),
            (
                DeviceState::Offline,
                DeviceState::Busy {
                    task_id: "t1".into(),
                },
            ),
            (DeviceState::Offline, DeviceState::Maintenance),
            (
                DeviceState::Maintenance,
                DeviceState::Busy {
                    task_id: "t1".into(),
                },
            ),
            (
                DeviceState::Busy {
                    task_id: "t1".into(),
                },
                DeviceState::Maintenance,
            ),
            (
                DeviceState::Error { reason: "x".into() },
                DeviceState::Busy {
                    task_id: "t1".into(),
                },
            ),
        ];
        for (from, to) in cases {
            assert!(
                !from.can_transition_to(&to),
                "{from:?} → {to:?} should be invalid"
            );
        }
    }

    #[test]
    fn device_state_is_available() {
        assert!(DeviceState::Ready.is_available());
        assert!(!DeviceState::Registering.is_available());
        assert!(
            !DeviceState::Busy {
                task_id: "t".into()
            }
            .is_available()
        );
        assert!(!DeviceState::Error { reason: "x".into() }.is_available());
        assert!(!DeviceState::Maintenance.is_available());
        assert!(!DeviceState::Offline.is_available());
    }

    #[test]
    fn device_state_as_str() {
        assert_eq!(DeviceState::Registering.as_str(), "registering");
        assert_eq!(DeviceState::Ready.as_str(), "ready");
        assert_eq!(
            DeviceState::Busy {
                task_id: "t".into()
            }
            .as_str(),
            "busy"
        );
        assert_eq!(DeviceState::Error { reason: "x".into() }.as_str(), "error");
        assert_eq!(DeviceState::Maintenance.as_str(), "maintenance");
        assert_eq!(DeviceState::Offline.as_str(), "offline");
    }
}
