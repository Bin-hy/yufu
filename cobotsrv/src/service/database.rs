use crate::config;
use anyhow::Result;
use log::info;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use std::time::Duration;

#[derive(Clone)]
pub struct DatabaseService {
    pub connection: DatabaseConnection,
}

impl DatabaseService {
    pub async fn new(db_config: &config::Database) -> Result<Self> {
        let mut opt = ConnectOptions::new(&db_config.url);
        opt.max_connections(db_config.max_connections)
            .connect_timeout(Duration::from_secs(db_config.connect_timeout))
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(3600))
            .sqlx_logging(true);
        info!("Connecting to database {}", db_config.url);
        let connection = Database::connect(opt).await?;

        Ok(Self { connection })
    }
}
