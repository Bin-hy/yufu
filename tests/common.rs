use auth::token_manager::TokenManager;
use cobotsrv::entity::role::RoleEnum;
use cobotsrv::error::AppError;
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
    let image_postgres = GenericImage::new("postgres", "16-alpine")
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

#[allow(dead_code)]
pub fn get_test_token(secret: String) -> String {
    let token_manager = TokenManager {
        secret,
        expire_seconds: 60 * 60 * 24 * 365 * 100,
    };
    token_manager
        .generate_token(RoleEnum::Superadmin.default_id())
        .unwrap()
}
