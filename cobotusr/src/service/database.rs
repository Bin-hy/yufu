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
        let redacted_url = redact_database_url(&db_config.url);
        println!("Connecting to database: {}", redacted_url);
        let mut opt = ConnectOptions::new(&db_config.url);
        opt.max_connections(db_config.max_connections)
            .connect_timeout(Duration::from_secs(db_config.connect_timeout))
            .idle_timeout(Duration::from_secs(600)) // idle duration before closing a connection 关闭连接前的空闲持续时间
            .max_lifetime(Duration::from_secs(3600)) // Set the maximum lifetime of individual connections 单个连接的最长生存期
            .sqlx_logging(true); // 开启SQLx 语句日志记录
        info!("Connecting to database {}", redacted_url);
        let connection = Database::connect(opt).await?;

        Ok(Self { connection })
    }
}

fn redact_database_url(raw: &str) -> String {
    let Ok(mut url) = url::Url::parse(raw) else {
        return "<redacted database url>".to_string();
    };
    if url.password().is_some() {
        let _ = url.set_password(Some("REDACTED"));
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_database_url_masks_password() {
        let redacted = redact_database_url("postgres://postgres:secret@localhost:5432/cobotrix");

        assert_eq!(
            redacted,
            "postgres://postgres:REDACTED@localhost:5432/cobotrix"
        );
        assert!(!redacted.contains("secret"));
    }

    #[test]
    fn test_redact_database_url_leaves_url_without_password_unchanged() {
        let url = "postgres://postgres@localhost:5432/cobotrix";

        assert_eq!(redact_database_url(url), url);
    }

    #[test]
    fn test_redact_database_url_hides_unparseable_url() {
        let raw = "not a url: with secret";

        assert_eq!(redact_database_url(raw), "<redacted database url>");
        assert!(!redact_database_url(raw).contains("secret"));
    }
}
