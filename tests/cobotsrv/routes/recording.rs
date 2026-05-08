use super::super::common::*;
use cobotsrv::config::Config;
use cobotsrv::entity::recording::RecordingVO;
use cobotsrv::entity::role::RoleEnum;
use cobotsrv::entity::{PageSelectQuery, PageSelectQueryResult};
use cobotsrv::error::AppError;
use reqwest::Client;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;

#[tokio::test]
async fn test_get_recording_by_page() -> Result<(), AppError> {
    // 运行一个docker镜像
    let (_pg, db_url) = get_postgres_container().await?;
    // 先注册一个用户
    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    let token = get_test_token(&mut cfg);
    tokio::spawn(cobotsrv::serve(cfg.clone(), listener, shutdown_signal()));

    let api_prefix = cfg.http.api_prefix;
    let uri = "/recordings?page_number=1&page_size=10";

    let client = Client::new();
    let resp = client
        .get(format!("http://{}{}{}", addr, api_prefix, uri))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    let result: PageSelectQueryResult<RecordingVO> = resp.json().await?;
    assert_eq!(result.page_number, 1);
    assert_eq!(result.page_size, 10);

    Ok(())
}

#[tokio::test]
async fn test_get_recording_by_user_id() -> Result<(), AppError> {
    // 运行一个docker镜像
    let (_pg, db_url) = get_postgres_container().await?;
    // 先注册一个用户
    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    let token = get_test_token(&mut cfg);
    tokio::spawn(cobotsrv::serve(cfg.clone(), listener, shutdown_signal()));

    // 先注册一个用户
    let api_prefix = cfg.http.api_prefix;

    let client = Client::new();

    // 然后查询该用户的操作记录
    let uri = format!("/recordings/{}", RoleEnum::Superadmin.default_id());

    let query_data = PageSelectQuery::default();
    let resp = client
        .get(format!("http://{}{}{}", addr, api_prefix, uri))
        .header("Authorization", format!("Bearer {}", token))
        .query(&query_data)
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());

    Ok(())
}
