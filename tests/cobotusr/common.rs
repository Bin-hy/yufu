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
    cfg.auth.allow_insecure_loopback_cookies = true;
    cfg.http.listen.set_ip(std::net::Ipv4Addr::LOCALHOST.into());
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

/// Disposable OP fixture. It owns its database, per-instance RSA key, listener,
/// clock and shutdown signal; no business database settings are read.
pub struct IdentityFixture {
    pub config: Config,
    pub db: sea_orm::DatabaseConnection,
    pub base_url: String,
    pub client: reqwest::Client,
    pub signer: std::sync::Arc<IdTokenSigner>,
    pub clock: std::sync::Arc<SecurityTestClock>,
    pub key_path: std::path::PathBuf,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    server: Option<tokio::task::JoinHandle<()>>,
    container: Option<testcontainers::ContainerAsync<GenericImage>>,
}

impl IdentityFixture {
    pub async fn start() -> Self {
        Self::start_configured(|_| {}).await
    }
    pub async fn start_configured(configure: impl FnOnce(&mut Config)) -> Self {
        use cobotusr::migration::MigratorTrait;
        use rsa::pkcs8::EncodePrivateKey;
        let (container, database_url) = get_postgres_container().await.unwrap();
        let db = sea_orm::Database::connect(database_url.clone())
            .await
            .unwrap();
        cobotusr::migration::Migrator::up(&db, None).await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let base_url = format!("http://{address}");
        let pem = tokio::task::spawn_blocking(|| {
            rsa::PrivateKey::new(&mut getrandom::SysRng, 2048)
                .unwrap()
                .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
                .unwrap()
        })
        .await
        .unwrap();
        let signer = std::sync::Arc::new(IdTokenSigner::from_rsa_pem(pem.as_bytes(), 300).unwrap());
        let key_path =
            std::env::temp_dir().join(format!("cobot-identity-{}.pem", uuid::Uuid::new_v4()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        use std::io::Write;
        options
            .open(&key_path)
            .unwrap()
            .write_all(pem.as_bytes())
            .unwrap();
        let mut config = Config::default();
        config.http.listen = address;
        config.database.url = database_url;
        config.oidc.issuer = base_url.clone();
        config.oidc.signing_algorithm = "RS256".into();
        config.oidc.rsa_private_key_path = key_path.to_string_lossy().into_owned();
        config.auth.allow_insecure_loopback_cookies = true;
        config.identity.background_workers_enabled = false;
        // Identity CORS must remain controlled even with permissive legacy CORS.
        config.auth.origins = vec!["*".into()];
        config.auth.secret = uuid::Uuid::new_v4().to_string();
        config.auth.exp_seconds = 300;
        config.auth.refresh_exp_seconds = 3600;
        config.auth.super_admin.username = "identity-admin".into();
        config.auth.super_admin.password = "fixture-admin-password".into();
        configure(&mut config);
        let clock = std::sync::Arc::new(SecurityTestClock::new(
            chrono::Utc::now().timestamp_millis() as u64,
        ));
        let (shutdown, stopped) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(cobotusr::serve_with_security_clock(
            config.clone(),
            listener,
            async {
                let _ = stopped.await;
            },
            clock.clone(),
        ));
        let fixture = Self {
            config,
            db,
            base_url,
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                // bcrypt cost 12 can take several seconds per hash on a slow
                // or loaded host. Password hashing has its own 10s CPU
                // allowance, so the test client must not impose a shorter
                // deadline than the server's own budget.
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
            signer,
            clock,
            key_path,
            shutdown: Some(shutdown),
            server: Some(server),
            container: Some(container),
        };
        let ready = tokio::time::timeout(std::time::Duration::from_secs(20), async {
            loop {
                assert!(
                    !fixture.server.as_ref().unwrap().is_finished(),
                    "OP exited before readiness"
                );
                if fixture
                    .client
                    .get(format!(
                        "{}/.well-known/openid-configuration",
                        fixture.base_url
                    ))
                    .send()
                    .await
                    .is_ok_and(|r| r.status().is_success())
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        })
        .await;
        assert!(ready.is_ok(), "isolated OP did not become ready");
        fixture
    }

    pub async fn close(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(server) = self.server.take() {
            tokio::time::timeout(std::time::Duration::from_secs(5), server)
                .await
                .unwrap()
                .unwrap();
        }
        self.db.clone().close().await.unwrap();
        if let Some(container) = self.container.take() {
            container.rm().await.unwrap();
        }
    }
}

impl Drop for IdentityFixture {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(server) = self.server.take() {
            server.abort();
        }
        let _ = std::fs::remove_file(&self.key_path);
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

/// Build historical data explicitly, outside the production bootstrap path.
pub async fn seed_legacy_bootstrap(f: &IdentityFixture) {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let owner: i32 =
        f.db.query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id FROM \"user\" WHERE username=$1",
            [f.config.auth.super_admin.username.clone().into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap();
    for entry in &f.config.oidc.bootstrap_clients {
        let secret_hash = if entry.public_client {
            String::new()
        } else {
            bcrypt::hash(
                entry.client_secret.as_deref().unwrap_or("fixture-secret"),
                bcrypt::DEFAULT_COST,
            )
            .unwrap()
        };
        let row = f.db.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "INSERT INTO oauth_client (client_id,user_id,name,redirect_uris,allowed_scopes,client_secret_hash,public_client,grant_types,create_time,update_time) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,NOW(),NOW()) RETURNING id",
            vec![entry.client_id.clone().into(),owner.into(),entry.name.clone().into(),entry.redirect_uris.join(",").into(),entry.allowed_scopes.join(",").into(),secret_hash.into(),entry.public_client.into(),entry.grant_types.join(",").into()]))
            .await.unwrap().unwrap();
        let id: i32 = row.try_get("", "id").unwrap();
        f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "INSERT INTO application (app_key,name,oidc_client_id,create_time,update_time) VALUES ($1,$2,$3,NOW(),NOW())",
            vec![entry.client_id.clone().into(),entry.name.clone().into(),id.into()])).await.unwrap();
    }
}
