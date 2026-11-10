//! Upgrade and restore drill for an existing deployment (gap G4).
//!
//! The drill builds a database at the schema of the release before the
//! identity work, seeds legacy rows, takes a `pg_dump` backup, upgrades with
//! the same migration path the service runs at startup, then restores both the
//! pre-upgrade and the post-upgrade dump into fresh databases and checks that
//! the service still starts, keeps the signing key and accepts a login.
//!
//! It is ignored by default: it needs Docker, takes a few minutes and prints
//! timings meant for the runbook in `docs/operations/enterprise-identity-upgrade.md`.
//!
//! ```text
//! cargo test --release --test mod upgrade_drill -- --ignored --nocapture --test-threads=1
//! ```

use super::super::common::*;
use auth::security::csrf::CsrfProtector;
use cobotusr::config::Config;
use cobotusr::migration::{Migrator, MigratorTrait};
use cobotusr::route::auth::LoginData;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use std::time::{Duration, Instant};
use testcontainers::GenericImage;
use testcontainers::core::{CmdWaitFor, ExecCommand};

const LEGACY_USERS: [&str; 2] = ["legacy-alice", "legacy-bob"];
const LEGACY_OWNER: &str = "legacy-owner";
const LEGACY_APP: &str = "legacy-crm";
const LEGACY_CLIENT: &str = "legacy-crm-web";
const DRILL_ADMIN: &str = "drill-admin";
const DRILL_PASSWORD: &str = "drill admin password";

/// Run a command inside the database container through the same client the
/// fixture uses, so the drill works whatever docker endpoint testcontainers
/// talks to.
async fn container_exec(
    container: &testcontainers::ContainerAsync<GenericImage>,
    args: &[&str],
) -> String {
    let mut result = container
        .exec(ExecCommand::new(args.to_vec()).with_cmd_ready_condition(CmdWaitFor::exit()))
        .await
        .unwrap();
    let stdout = result.stdout_to_vec().await.unwrap();
    let stderr = result.stderr_to_vec().await.unwrap();
    let code = result.exit_code().await.unwrap().unwrap_or(-1);
    assert_eq!(
        code,
        0,
        "container exec {args:?} failed: {}",
        String::from_utf8_lossy(&stderr)
    );
    String::from_utf8_lossy(&stdout).trim().to_string()
}

fn database_name(url: &str) -> String {
    url.rsplit('/')
        .next()
        .expect("database url has a name")
        .into()
}

async fn scalar(db: &DatabaseConnection, sql: &str) -> i64 {
    let row = db
        .query_one_raw(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await
        .unwrap()
        .expect("query returns a row");
    row.try_get::<i64>("", "value").unwrap()
}

async fn table_exists(db: &DatabaseConnection, table: &str) -> bool {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT to_regclass($1) IS NOT NULL AS value",
            [format!("public.{table}").into()],
        ))
        .await
        .unwrap()
        .unwrap();
    row.try_get::<bool>("", "value").unwrap()
}

/// The schema of the release before the identity work: every migration up to,
/// but not including, `m20261107_000001_security_identity`.
fn pre_identity_steps() -> u32 {
    Migrator::migrations()
        .iter()
        .position(|migration| migration.name().starts_with("m20261107_000001"))
        .expect("the identity migrations are part of the migrator") as u32
}

async fn seed_legacy_rows(db: &DatabaseConnection) {
    let hash = tokio::task::spawn_blocking(|| {
        bcrypt::hash("legacy password", bcrypt::DEFAULT_COST).unwrap()
    })
    .await
    .unwrap();
    // The legacy list endpoint hides the account with id 1 (the legacy
    // superadmin id), so the client owner takes that slot.
    for (index, name) in [LEGACY_OWNER].iter().chain(LEGACY_USERS.iter()).enumerate() {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"INSERT INTO "user" (username,password,nickname,role_id,description,create_time,update_time)
               VALUES ($1,$2,$3,2,$4,NOW(),NOW())"#,
            vec![
                (*name).into(),
                hash.clone().into(),
                format!("{name} nickname").into(),
                format!("legacy user {}", index + 1).into(),
            ],
        ))
        .await
        .unwrap();
    }
    db.execute_unprepared(&format!(
        r#"INSERT INTO application (app_key,name,create_time,update_time)
           VALUES ('{LEGACY_APP}','Legacy CRM',NOW(),NOW());
           INSERT INTO oauth_client (client_id,user_id,name,redirect_uris,allowed_scopes,
               client_secret_hash,public_client,grant_types,create_time,update_time)
           SELECT '{LEGACY_CLIENT}',id,'Legacy CRM Web','https://legacy.example/callback',
               'openid profile','',TRUE,'authorization_code',NOW(),NOW()
           FROM "user" WHERE username='{LEGACY_OWNER}';"#
    ))
    .await
    .unwrap();
}

fn drill_config(
    db_url: &str,
    key_path: &std::path::Path,
    address: std::net::SocketAddr,
    issuer: &str,
) -> Config {
    let mut config = Config::default();
    config.http.listen = address;
    config.database.url = db_url.to_owned();
    config.oidc.issuer = issuer.into();
    config.oidc.signing_algorithm = "RS256".into();
    config.oidc.rsa_private_key_path = key_path.to_string_lossy().into_owned();
    config.auth.allow_insecure_loopback_cookies = true;
    config.identity.background_workers_enabled = false;
    config.auth.origins = vec!["*".into()];
    config.auth.secret = "upgrade-drill-csrf-key".into();
    config.auth.exp_seconds = 300;
    config.auth.refresh_exp_seconds = 3600;
    config.auth.super_admin.username = DRILL_ADMIN.into();
    config.auth.super_admin.password = DRILL_PASSWORD.into();
    config
}

struct DrillService {
    config: Config,
    base_url: String,
    client: reqwest::Client,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    server: Option<tokio::task::JoinHandle<()>>,
}

impl DrillService {
    async fn start(
        db_url: &str,
        key_path: &std::path::Path,
        listener: tokio::net::TcpListener,
        issuer: &str,
    ) -> (Self, Duration) {
        let address = listener.local_addr().unwrap();
        let config = drill_config(db_url, key_path, address, issuer);
        let clock = std::sync::Arc::new(SecurityTestClock::new(
            chrono::Utc::now().timestamp_millis() as u64,
        ));
        let (shutdown, stopped) = tokio::sync::oneshot::channel();
        let started = Instant::now();
        let server = tokio::spawn(cobotusr::serve_with_security_clock(
            config.clone(),
            listener,
            async {
                let _ = stopped.await;
            },
            clock,
        ));
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let base_url = format!("http://{address}");
        let ready = tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                assert!(!server.is_finished(), "service exited before readiness");
                if client
                    .get(format!("{base_url}/.well-known/openid-configuration"))
                    .send()
                    .await
                    .is_ok_and(|response| response.status().is_success())
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;
        assert!(ready.is_ok(), "service did not become ready");
        (
            Self {
                config,
                base_url,
                client,
                shutdown: Some(shutdown),
                server: Some(server),
            },
            started.elapsed(),
        )
    }

    async fn discovery(&self) -> serde_json::Value {
        self.client
            .get(format!(
                "{}/.well-known/openid-configuration",
                self.base_url
            ))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    /// Modulus of the first JWKS key; identical before and after a restore
    /// means the signing key survived the backup.
    async fn key_modulus(&self) -> String {
        let jwks: serde_json::Value = self
            .client
            .get(format!("{}/.well-known/jwks.json", self.base_url))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        jwks["keys"][0]["n"]
            .as_str()
            .expect("jwks has a modulus")
            .into()
    }

    async fn login(&self, username: &str, password: &str) -> reqwest::Response {
        let token = CsrfProtector::new(self.config.auth.secret.clone())
            .unwrap()
            .issue(
                &self.config.oidc.issuer,
                None,
                &SecurityTestClock::new(chrono::Utc::now().timestamp_millis() as u64),
            )
            .unwrap();
        self.client
            .post(format!(
                "{}{}/login",
                self.base_url, self.config.http.api_prefix
            ))
            .header("origin", &self.config.oidc.issuer)
            .header("x-csrf-token", token.expose_secret())
            .header(
                "cookie",
                format!("cobotusr-dev-csrf={}", token.expose_secret()),
            )
            .json(&LoginData {
                username: username.into(),
                password: password.into(),
            })
            .send()
            .await
            .unwrap()
    }

    async fn users_page(&self, access_token: &str) -> String {
        self.client
            .get(format!(
                "{}{}/users?page_size=10&page_number=1",
                self.base_url, self.config.http.api_prefix
            ))
            .bearer_auth(access_token)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .text()
            .await
            .unwrap()
    }

    async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(server) = self.server.take() {
            tokio::time::timeout(Duration::from_secs(10), server)
                .await
                .unwrap()
                .unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs Docker and runs for minutes; see the module docs"]
async fn upgrade_drill_backup_migrate_and_restore_round_trip() {
    let (container, url) = get_postgres_container().await.unwrap();
    let db = sea_orm::Database::connect(url.clone()).await.unwrap();
    let db_name = database_name(&url);
    let restored_name = format!("{db_name}_restored");
    let rollback_name = format!("{db_name}_rollback");
    let mut report: Vec<(String, String)> = vec![];

    // 1. Old release schema.
    let steps = pre_identity_steps();
    let started = Instant::now();
    Migrator::up(&db, Some(steps)).await.unwrap();
    report.push((
        "old schema".into(),
        format!(
            "{steps} migrations in {:.2} s",
            started.elapsed().as_secs_f64()
        ),
    ));
    assert!(!table_exists(&db, "identity_subject").await);
    assert!(!table_exists(&db, "security_session").await);
    assert!(table_exists(&db, "user").await);

    // 2. Legacy business rows that must survive the upgrade.
    seed_legacy_rows(&db).await;
    let legacy_users = scalar(&db, r#"SELECT count(*) AS value FROM "user""#).await;
    let legacy_apps = scalar(&db, "SELECT count(*) AS value FROM application").await;
    let legacy_clients = scalar(&db, "SELECT count(*) AS value FROM oauth_client").await;
    assert_eq!(legacy_users, LEGACY_USERS.len() as i64 + 1);
    assert_eq!(legacy_apps, 1);
    assert_eq!(legacy_clients, 1);

    // 3. Backup before the maintenance window.
    let started = Instant::now();
    container_exec(
        &container,
        &[
            "pg_dump",
            "-U",
            "postgres",
            "-Fc",
            "--no-owner",
            "-f",
            "/tmp/pre.dump",
            &db_name,
        ],
    )
    .await;
    let pre_backup = started.elapsed();
    let pre_size = container_exec(&container, &["stat", "-c", "%s", "/tmp/pre.dump"]).await;
    report.push((
        "backup (pre-upgrade)".into(),
        format!("{pre_backup:.2?}, {pre_size} bytes"),
    ));

    // 4. Upgrade through the same path the service takes at startup.
    let started = Instant::now();
    Migrator::up(&db, None).await.unwrap();
    let upgrade = started.elapsed();
    report.push(("upgrade".into(), format!("{upgrade:.2?}")));
    let total_migrations = Migrator::migrations().len() as i64;
    assert_eq!(
        scalar(&db, "SELECT count(*) AS value FROM seaql_migrations").await,
        total_migrations
    );
    assert!(table_exists(&db, "identity_subject").await);
    assert!(table_exists(&db, "identity_policy_barrier").await);
    assert_eq!(
        scalar(&db, r#"SELECT count(*) AS value FROM "user""#).await,
        legacy_users
    );
    assert_eq!(
        scalar(&db, "SELECT count(*) AS value FROM application").await,
        legacy_apps
    );
    assert_eq!(
        scalar(&db, "SELECT count(*) AS value FROM oauth_client").await,
        legacy_clients
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT count(*) AS value FROM identity_policy_barrier WHERE id=1"
        )
        .await,
        1
    );

    // 5. The upgraded database serves the admin and the legacy accounts.
    let key_path = std::env::temp_dir().join(format!("drill-{}.pem", uuid::Uuid::new_v4()));
    let pem = tokio::task::spawn_blocking(|| {
        use rsa::pkcs8::EncodePrivateKey;
        rsa::PrivateKey::new(&mut getrandom::SysRng, 2048)
            .unwrap()
            .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap()
    })
    .await
    .unwrap();
    {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(&key_path)
            .unwrap()
            .write_all(pem.as_bytes())
            .unwrap();
    }
    // A fixed issuer is part of the product contract: the restored deployment
    // must reuse it, otherwise provider initialization reports a conflict.
    let upgraded_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upgraded_issuer = format!("http://{}", upgraded_listener.local_addr().unwrap());
    let (upgraded, ready) =
        DrillService::start(&url, &key_path, upgraded_listener, &upgraded_issuer).await;
    report.push(("service ready (upgraded)".into(), format!("{ready:.2?}")));
    assert_eq!(
        upgraded.discovery().await["issuer"],
        upgraded.config.oidc.issuer
    );
    let modulus = upgraded.key_modulus().await;
    let login = upgraded.login(DRILL_ADMIN, DRILL_PASSWORD).await;
    assert_eq!(login.status(), 200, "admin login on the upgraded database");
    let body: serde_json::Value = login.json().await.unwrap();
    let access = body["access_token"].as_str().unwrap().to_owned();
    let page = upgraded.users_page(&access).await;
    assert!(
        LEGACY_USERS.iter().all(|name| page.contains(name)),
        "legacy accounts stay readable after the upgrade: {page}"
    );
    upgraded.stop().await;

    // 6. Restore the upgraded dump into a fresh database.
    let started = Instant::now();
    container_exec(
        &container,
        &[
            "pg_dump",
            "-U",
            "postgres",
            "-Fc",
            "--no-owner",
            "-f",
            "/tmp/post.dump",
            &db_name,
        ],
    )
    .await;
    let post_backup = started.elapsed();
    container_exec(&container, &["createdb", "-U", "postgres", &restored_name]).await;
    let started = Instant::now();
    container_exec(
        &container,
        &[
            "pg_restore",
            "-U",
            "postgres",
            "--no-owner",
            "-d",
            &restored_name,
            "/tmp/post.dump",
        ],
    )
    .await;
    let restore = started.elapsed();
    report.push((
        "backup + restore (post-upgrade)".into(),
        format!("{post_backup:.2?} + {restore:.2?}"),
    ));
    let restored_url = url.replace(&db_name, &restored_name);
    let restored_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (restored, ready) = DrillService::start(
        &restored_url,
        &key_path,
        restored_listener,
        &upgraded_issuer,
    )
    .await;
    report.push(("service ready (restored)".into(), format!("{ready:.2?}")));
    assert_eq!(
        restored.discovery().await["issuer"],
        restored.config.oidc.issuer
    );
    assert_eq!(
        restored.key_modulus().await,
        modulus,
        "the restored deployment signs with the same key"
    );
    let login = restored.login(DRILL_ADMIN, DRILL_PASSWORD).await;
    assert_eq!(login.status(), 200, "admin login on the restored database");
    let body: serde_json::Value = login.json().await.unwrap();
    let page = restored
        .users_page(body["access_token"].as_str().unwrap())
        .await;
    assert!(LEGACY_USERS.iter().all(|name| page.contains(name)));
    let restored_db = sea_orm::Database::connect(restored_url.clone())
        .await
        .unwrap();
    assert_eq!(
        scalar(&restored_db, r#"SELECT count(*) AS value FROM "user""#).await,
        legacy_users + 1,
        "restore keeps the legacy accounts and the bootstrapped admin"
    );
    assert_eq!(
        scalar(
            &restored_db,
            "SELECT count(*) AS value FROM identity_audit_event"
        )
        .await,
        scalar(&db, "SELECT count(*) AS value FROM identity_audit_event").await
    );
    restored.stop().await;

    // 7. Failure recovery: the pre-upgrade dump still yields the old release.
    container_exec(&container, &["createdb", "-U", "postgres", &rollback_name]).await;
    let started = Instant::now();
    container_exec(
        &container,
        &[
            "pg_restore",
            "-U",
            "postgres",
            "--no-owner",
            "-d",
            &rollback_name,
            "/tmp/pre.dump",
        ],
    )
    .await;
    let rollback = started.elapsed();
    report.push(("rollback restore".into(), format!("{rollback:.2?}")));
    let rollback_db = sea_orm::Database::connect(url.replace(&db_name, &rollback_name))
        .await
        .unwrap();
    assert!(!table_exists(&rollback_db, "identity_subject").await);
    assert_eq!(
        scalar(&rollback_db, r#"SELECT count(*) AS value FROM "user""#).await,
        legacy_users
    );
    assert_eq!(
        scalar(&rollback_db, "SELECT count(*) AS value FROM application").await,
        legacy_apps
    );

    println!("\nupgrade drill (single machine, Docker Postgres)");
    for (step, value) in &report {
        println!("  {step:<32} {value}");
    }
    println!();

    let _ = std::fs::remove_file(&key_path);
    db.close().await.unwrap();
    restored_db.close().await.unwrap();
    rollback_db.close().await.unwrap();
    container.rm().await.unwrap();
}
