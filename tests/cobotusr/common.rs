use auth::token_manager::IdTokenSigner;
use cobotusr::config::Config;
use cobotusr::entity::role::RoleEnum;
use cobotusr::error::AppError;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{GenericImage, ImageExt};

pub async fn shutdown_signal() {
    let _str = signal::wait_for_stop_signal().await;
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
