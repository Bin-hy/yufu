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
        // https://www.sea-ql.org/SeaORM/docs/install-and-config/connection/#connect-options
        println!("Connecting to database: {}", db_config.url);
        let url = &db_config.url;
        let mut opt = ConnectOptions::new(&db_config.url);
        opt.max_connections(db_config.max_connections)
            .connect_timeout(Duration::from_secs(db_config.connect_timeout))
            .idle_timeout(Duration::from_secs(600)) // idle duration before closing a connection 关闭连接前的空闲持续时间
            .max_lifetime(Duration::from_secs(3600)) // Set the maximum lifetime of individual connections 单个连接的最长生存期
            .sqlx_logging(true); // 开启SQLx 语句日志记录
        info!("Connecting to database {}", url);
        let connection = Database::connect(opt).await?;

        Ok(Self { connection })
    }

    // 使用现有的数据库连接创建DatabaseService
    pub async fn new_with_connection(
        _db_config: &config::Database,
        connection: DatabaseConnection,
    ) -> Result<Self> {
        println!("Using existing database connection");
        info!("Database connection established");

        Ok(Self { connection })
    }

    pub fn get_connection(&self) -> &DatabaseConnection {
        &self.connection
    }
}
