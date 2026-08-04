use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};

use crate::dispatcher::{Task, TaskResult};

const TABLE: TableDefinition<&str, &str> = TableDefinition::new("tasks");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    #[default]
    Queued,
    Dispatched,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub task: Task,
    pub state: TaskState,
    pub device_id: Option<String>,
    pub attempts: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub result: Option<TaskResult>,
    pub fail_reason: Option<String>,
}

pub struct TaskStore {
    db: Arc<Database>,
}

impl TaskStore {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    fn persist(&self, record: &TaskRecord) -> Result<()> {
        let json = serde_json::to_string(record)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE)?;
            table.insert(record.id.as_str(), json.as_str())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    fn load(&self, id: &str) -> Result<Option<TaskRecord>> {
        let read_txn = self.db.begin_read()?;
        let table = match read_txn.open_table(TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        match table.get(id)? {
            Some(v) => Ok(Some(serde_json::from_str(v.value())?)),
            None => Ok(None),
        }
    }

    pub fn enqueue(&self, task: &Task) -> Result<TaskRecord> {
        let now = Utc::now();
        let record = TaskRecord {
            id: task.id.clone(),
            task: task.clone(),
            state: TaskState::Queued,
            device_id: None,
            attempts: 0,
            created_at: now,
            updated_at: now,
            result: None,
            fail_reason: None,
        };
        self.persist(&record)?;
        Ok(record)
    }

    pub fn mark_dispatched(&self, id: &str, device_id: &str) -> Result<()> {
        let Some(mut record) = self.load(id)? else {
            return Ok(());
        };
        record.state = TaskState::Dispatched;
        record.device_id = Some(device_id.to_string());
        record.attempts += 1;
        record.updated_at = Utc::now();
        self.persist(&record)
    }

    pub fn complete(&self, id: &str, result: &TaskResult) -> Result<()> {
        let Some(mut record) = self.load(id)? else {
            return Ok(());
        };
        record.state = TaskState::Completed;
        record.result = Some(result.clone());
        record.updated_at = Utc::now();
        self.persist(&record)
    }

    pub fn fail(&self, id: &str, reason: String, result: Option<&TaskResult>) -> Result<()> {
        let Some(mut record) = self.load(id)? else {
            return Ok(());
        };
        record.state = TaskState::Failed;
        record.fail_reason = Some(reason);
        record.result = result.cloned();
        record.updated_at = Utc::now();
        self.persist(&record)
    }

    pub fn get(&self, id: &str) -> Result<Option<TaskRecord>> {
        self.load(id)
    }

    pub fn list(
        &self,
        state_filter: Option<&TaskState>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<TaskRecord>> {
        let read_txn = self.db.begin_read()?;
        let table = match read_txn.open_table(TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut records: Vec<TaskRecord> = Vec::new();
        for entry in table.iter()? {
            let (_, v) = entry?;
            let record: TaskRecord = serde_json::from_str(v.value())?;
            if state_filter.is_some_and(|filter| &record.state != filter) {
                continue;
            }
            records.push(record);
        }
        records.sort_by_key(|r| std::cmp::Reverse(r.created_at));
        Ok(records.into_iter().skip(offset).take(limit).collect())
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let existed = {
            let mut table = write_txn.open_table(TABLE)?;
            table.remove(id)?.is_some()
        };
        write_txn.commit()?;
        Ok(existed)
    }

    pub fn delete_completed(&self) -> Result<u64> {
        let read_txn = self.db.begin_read()?;
        let table = match read_txn.open_table(TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(0),
            Err(e) => return Err(e.into()),
        };
        let mut to_delete = Vec::new();
        for entry in table.iter()? {
            let (k, v) = entry?;
            let record: TaskRecord = serde_json::from_str(v.value())?;
            if matches!(record.state, TaskState::Completed | TaskState::Failed) {
                to_delete.push(k.value().to_string());
            }
        }
        drop(table);
        drop(read_txn);

        let count = to_delete.len() as u64;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE)?;
            for id in &to_delete {
                table.remove(id.as_str())?;
            }
        }
        write_txn.commit()?;
        Ok(count)
    }

    pub fn recover(&self) -> Result<Vec<TaskRecord>> {
        let read_txn = self.db.begin_read()?;
        let table = match read_txn.open_table(TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut inflight = Vec::new();
        for entry in table.iter()? {
            let (_, v) = entry?;
            let record: TaskRecord = serde_json::from_str(v.value())?;
            if matches!(record.state, TaskState::Queued | TaskState::Dispatched) {
                inflight.push(record);
            }
        }
        drop(table);
        drop(read_txn);

        let now = Utc::now();
        for record in &mut inflight {
            record.state = TaskState::Failed;
            record.fail_reason = Some("process restart".to_string());
            record.updated_at = now;
            self.persist(record)?;
        }
        Ok(inflight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_store() -> (TaskStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::create(dir.path().join("test.redb")).unwrap());
        (TaskStore::new(db), dir)
    }

    fn sample_task(id: &str) -> Task {
        Task {
            id: id.to_string(),
            name: "move".to_string(),
            device_id: None,
            input: None,
            headers: HashMap::new(),
            timeout_ms: Some(5000),
        }
    }

    #[test]
    fn enqueue_and_get() {
        let (ts, _d) = make_store();
        let record = ts.enqueue(&sample_task("t1")).unwrap();
        assert_eq!(record.state, TaskState::Queued);
        assert_eq!(record.id, "t1");

        let loaded = ts.get("t1").unwrap().unwrap();
        assert_eq!(loaded.state, TaskState::Queued);
        assert_eq!(loaded.task.name, "move");
    }

    #[test]
    fn lifecycle_queued_dispatched_completed() {
        let (ts, _d) = make_store();
        ts.enqueue(&sample_task("t1")).unwrap();

        ts.mark_dispatched("t1", "dev-1").unwrap();
        let r = ts.get("t1").unwrap().unwrap();
        assert_eq!(r.state, TaskState::Dispatched);
        assert_eq!(r.device_id.as_deref(), Some("dev-1"));
        assert_eq!(r.attempts, 1);

        let result = TaskResult {
            id: "t1".into(),
            is_error: false,
            content: vec![],
            status: 200,
        };
        ts.complete("t1", &result).unwrap();
        let r = ts.get("t1").unwrap().unwrap();
        assert_eq!(r.state, TaskState::Completed);
        assert!(r.result.is_some());
        assert_eq!(r.result.unwrap().status, 200);
    }

    #[test]
    fn fail_with_reason() {
        let (ts, _d) = make_store();
        ts.enqueue(&sample_task("t1")).unwrap();
        ts.fail("t1", "device offline".into(), None).unwrap();

        let r = ts.get("t1").unwrap().unwrap();
        assert_eq!(r.state, TaskState::Failed);
        assert_eq!(r.fail_reason.as_deref(), Some("device offline"));
        assert!(r.result.is_none());
    }

    #[test]
    fn recover_marks_inflight_failed() {
        let (ts, _d) = make_store();
        ts.enqueue(&sample_task("t1")).unwrap();
        ts.enqueue(&sample_task("t2")).unwrap();
        ts.mark_dispatched("t2", "dev-1").unwrap();
        // t3 is completed — should not be affected
        ts.enqueue(&sample_task("t3")).unwrap();
        ts.complete(
            "t3",
            &TaskResult {
                id: "t3".into(),
                is_error: false,
                content: vec![],
                status: 200,
            },
        )
        .unwrap();

        let recovered = ts.recover().unwrap();
        assert_eq!(recovered.len(), 2);

        let t1 = ts.get("t1").unwrap().unwrap();
        assert_eq!(t1.state, TaskState::Failed);
        assert_eq!(t1.fail_reason.as_deref(), Some("process restart"));

        let t2 = ts.get("t2").unwrap().unwrap();
        assert_eq!(t2.state, TaskState::Failed);
        assert_eq!(t2.fail_reason.as_deref(), Some("process restart"));

        let t3 = ts.get("t3").unwrap().unwrap();
        assert_eq!(t3.state, TaskState::Completed);
    }

    #[test]
    fn list_with_filter() {
        let (ts, _d) = make_store();
        ts.enqueue(&sample_task("t1")).unwrap();
        ts.enqueue(&sample_task("t2")).unwrap();
        ts.fail("t2", "err".into(), None).unwrap();
        ts.enqueue(&sample_task("t3")).unwrap();
        ts.complete(
            "t3",
            &TaskResult {
                id: "t3".into(),
                is_error: false,
                content: vec![],
                status: 200,
            },
        )
        .unwrap();

        let all = ts.list(None, 100, 0).unwrap();
        assert_eq!(all.len(), 3);

        let failed = ts.list(Some(&TaskState::Failed), 100, 0).unwrap();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].id, "t2");

        let queued = ts.list(Some(&TaskState::Queued), 100, 0).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].id, "t1");
    }

    #[test]
    fn list_pagination() {
        let (ts, _d) = make_store();
        for i in 0..5 {
            ts.enqueue(&sample_task(&format!("t{i}"))).unwrap();
        }
        let page = ts.list(None, 2, 1).unwrap();
        assert_eq!(page.len(), 2);
    }

    #[test]
    fn delete_single() {
        let (ts, _d) = make_store();
        ts.enqueue(&sample_task("t1")).unwrap();
        assert!(ts.delete("t1").unwrap());
        assert!(!ts.delete("t1").unwrap());
        assert!(ts.get("t1").unwrap().is_none());
    }

    #[test]
    fn delete_completed_only() {
        let (ts, _d) = make_store();
        ts.enqueue(&sample_task("t1")).unwrap(); // queued
        ts.enqueue(&sample_task("t2")).unwrap();
        ts.fail("t2", "err".into(), None).unwrap(); // failed
        ts.enqueue(&sample_task("t3")).unwrap();
        ts.complete(
            "t3",
            &TaskResult {
                id: "t3".into(),
                is_error: false,
                content: vec![],
                status: 200,
            },
        )
        .unwrap(); // completed

        let deleted = ts.delete_completed().unwrap();
        assert_eq!(deleted, 2); // t2 (failed) + t3 (completed)

        // t1 (queued) remains
        assert!(ts.get("t1").unwrap().is_some());
        assert!(ts.get("t2").unwrap().is_none());
        assert!(ts.get("t3").unwrap().is_none());
    }
}
