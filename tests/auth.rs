use cobotsrv::config::Config;
use reqwest::Client;
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;
mod common;
use common::*;
#[derive(Serialize)]
struct LoginData {
    username: String,
    password: String,
}

#[tokio::test]
async fn test_login() -> anyhow::Result<()> {
    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let cfg = Config::default();

    tokio::spawn(cobotsrv::serve(cfg.clone(), listener, shutdown_signal()));

    let api_prefix = cfg.http.api_prefix;
    let uri = "/login";

    let client = Client::new();
    // build body
    let body = LoginData {
        username: "sadmin".to_string(),
        password: "sadmin".to_string(),
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
async fn test_register() -> anyhow::Result<()> {
    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let cfg = Config::default();
    tokio::spawn(cobotsrv::serve(cfg.clone(), listener, shutdown_signal()));

    let api_prefix = cfg.http.api_prefix;
    let uri = "/login";

    let client = Client::new();
    // build body
    let body = LoginData {
        username: "sadmin".to_string(),
        password: "sadmin".to_string(),
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
