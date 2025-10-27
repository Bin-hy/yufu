use auth::token_manager::TokenManager;
use cobotsrv::entity::role::RoleEnum;
use cobotsrv::error::AppError;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

pub async fn shutdown_signal() {
    let _str = signal::wait_for_stop_signal().await;
}

pub async fn get_postgres_container()
-> Result<(testcontainers::ContainerAsync<Postgres>, String), AppError> {
    // 启动 Postgres 模块容器
    let container = Postgres::default().start().await?;

    // 获取主机映射端口
    let host_port = container.get_host_port_ipv4(5432).await?;

    // 构造数据库 URL
    let db_url = format!(
        "postgres://postgres:postgres@localhost:{}/postgres",
        host_port
    );

    Ok((container, db_url))
}

pub fn get_test_token(secret: String) -> String {
    let token_manager = TokenManager {
        secret,
        expire_seconds: 60 * 60 * 24 * 365 * 100,
    };
    token_manager
        .generate_token(RoleEnum::Superadmin.default_id())
        .unwrap()
}
