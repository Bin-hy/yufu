use auth::token_manager::IdTokenSigner;
use cobotusr::config::Config;
use cobotusr::entity::role::RoleEnum;
use cobotusr::error::AppError;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{GenericImage, ImageExt};

pub async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[allow(dead_code)]
pub async fn get_postgres_container()
-> Result<(testcontainers::ContainerAsync<GenericImage>, String), AppError> {
    let db_name = format!("test_db_{}", uuid::Uuid::new_v4());
    let image_postgres = GenericImage::new("postgres", "18-alpine")
        .with_exposed_port(5432.tcp())
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .with_env_var("POSTGRES_DB", db_name.clone());
    let container = image_postgres.start().await?;

    let host_port = container.get_host_port_ipv4(5432.tcp()).await?;
    let db_url = format!(
        "postgres://postgres:postgres@localhost:{}/{}",
        host_port, db_name
    );
    Ok((container, db_url))
}

/// Mint a long-lived superadmin access token for tests, AND force the test
/// server config to use HS256 (HMAC with `auth.secret`) so it accepts what we sign.
///
/// Production servers default to RS256 with an RSA key file, which would require
/// generating + sharing a key per test. HMAC mode keeps tests fast and self-contained.
#[allow(dead_code)]
pub fn get_test_token(cfg: &mut Config) -> String {
    cfg.oidc.signing_algorithm = "HS256".to_string();
    let signer = IdTokenSigner::from_hmac(&cfg.auth.secret, cfg.auth.exp_seconds);
    signer
        .sign_access_token(
            &RoleEnum::Superadmin.default_id().to_string(),
            "cobotusr-test",
            "cobotusr",
            "",
            60 * 60 * 24 * 365 * 100,
            vec![],
            vec![],
        )
        .unwrap()
}

/// Independently controllable wall and monotonic clocks for deadline/fault scenarios.
pub struct SecurityTestClock {
    reading: std::sync::Mutex<auth::security::budget::ClockReading>,
}

impl SecurityTestClock {
    pub fn new(unix_millis: u64) -> Self {
        Self {
            reading: std::sync::Mutex::new(auth::security::budget::ClockReading {
                monotonic: std::time::Duration::ZERO,
                unix_millis,
                synchronized: true,
            }),
        }
    }

    pub fn advance(&self, millis: u64) {
        let mut reading = self.reading.lock().unwrap();
        reading.monotonic += std::time::Duration::from_millis(millis);
        reading.unix_millis = reading.unix_millis.checked_add(millis).unwrap();
    }

    pub fn set_wall_time(&self, unix_millis: u64) {
        self.reading.lock().unwrap().unix_millis = unix_millis;
    }

    pub fn set_synchronized(&self, synchronized: bool) {
        self.reading.lock().unwrap().synchronized = synchronized;
    }
}

impl auth::security::budget::Clock for SecurityTestClock {
    fn now(
        &self,
    ) -> Result<auth::security::budget::ClockReading, auth::security::error::SecurityError> {
        Ok(*self.reading.lock().unwrap())
    }
}

/// Explicit test application; production migrations never grant business access.
pub async fn seed_resource_application(db: &sea_orm::DatabaseConnection) {
    use sea_orm::ConnectionTrait;
    db.execute_unprepared(r#"
        INSERT INTO application (app_key,name,create_time,update_time)
        VALUES ('resource-app','Test resource application',NOW(),NOW()) ON CONFLICT (app_key) DO NOTHING;
        INSERT INTO permission (application_id,name,action,resource_type,resource_pattern,create_time,update_time)
        SELECT id,'mcp:access','mcp:access','application','{"application":"resource-app","scope":{"scope":"application","kind":"application"}}',NOW(),NOW()
        FROM application WHERE app_key='resource-app' AND NOT EXISTS (SELECT 1 FROM permission WHERE name='mcp:access' AND application_id=application.id);
        INSERT INTO role_permission (role_id,permission_id,create_time)
        SELECT 3,p.id,NOW() FROM permission p JOIN application a ON a.id=p.application_id
        WHERE a.app_key='resource-app' AND p.name='mcp:access' ON CONFLICT DO NOTHING;
    "#).await.unwrap();
}
