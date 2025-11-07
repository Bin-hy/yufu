use cobotsrv::config::Config;
use cobotsrv::error::AppError;
use reqwest::Client;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;

mod common;
use common::*;

use cobotsrv::route::auth::{LoginData, RegistryData};

#[tokio::test]
async fn test_register() -> Result<(), AppError> {
    // 运行一个docker镜像
    let (_pg, db_url) = get_postgres_container().await?;

    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    tokio::spawn(cobotsrv::serve(cfg.clone(), listener, shutdown_signal()));

    let api_prefix = cfg.http.api_prefix;
    let uri = "/register";

    let client = Client::new();
    // build body
    let body = RegistryData {
        username: "test_user".to_string(),
        nickname: "test_user".to_string(),
        password: "test_user".to_string(),
    };

    let resp = client
        .post(format!("http://{}{}{}", addr, api_prefix, uri))
        .json(&body)
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    Ok(())
}

#[tokio::test]
async fn test_login() -> Result<(), AppError> {
    // 运行一个docker镜像
    let (_pg, db_url) = get_postgres_container().await?;

    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    tokio::spawn(cobotsrv::serve(cfg.clone(), listener, shutdown_signal()));

    let api_prefix = cfg.http.api_prefix;

    // 1. 注册用户
    let uri_register = "/register";
    let client_register = Client::new();
    let body_register = RegistryData {
        username: "test_user".to_string(),
        nickname: "test_user".to_string(),
        password: "test_user".to_string(),
    };
    let resp_register = client_register
        .post(format!(
            "http://{}{}{}",
            addr.clone(),
            api_prefix,
            uri_register
        ))
        .json(&body_register)
        .send()
        .await?
        .error_for_status()?;
    assert!(resp_register.status().is_success());

    // 2. 登陆用户
    let uri = "/login";

    let client = Client::new();
    // build body
    let body = LoginData {
        username: "test_user".to_string(),
        password: "test_user".to_string(),
    };
    let resp = client
        .post(format!("http://{}{}{}", addr, api_prefix, uri))
        .json(&body)
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    Ok(())
}
