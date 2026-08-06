pub use sea_orm_migration::prelude::*;

mod m20260506_000001_create_device_table;
mod m20260602_000001_create_easytier_network;
mod m20260616_000001_create_mcp_call_log;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260506_000001_create_device_table::Migration),
            Box::new(m20260602_000001_create_easytier_network::Migration),
            Box::new(m20260616_000001_create_mcp_call_log::Migration),
        ]
    }
}
