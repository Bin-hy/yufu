pub mod device;
pub mod task;

use std::path::Path;

use anyhow::Result;
use redb::Database;

pub use device::{Capability, Device, DeviceState, DeviceStore};
pub use task::{TaskRecord, TaskState, TaskStore};

pub fn open_database(path: impl AsRef<Path>) -> Result<Database> {
    let path = path.as_ref();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(Database::create(path)?)
}
