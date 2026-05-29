use super::super::common::*;
use cobotusr::config::Config;
use cobotusr::entity::authority::{
    AuthorityDeleteReq, AuthorityInsertReq, AuthorityPatchReq, AuthorityVO,
};
use cobotusr::error::AppError;
use reqwest::Client;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;

#[tokio::test]
async fn test_add_authority() -> Result<(), AppError> {
    // 运行一个docker镜像
    let (_pg, db_url) = get_postgres_container().await?;

    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    let token = get_test_token(&mut cfg);
    tokio::spawn(cobotusr::serve(cfg.clone(), listener, shutdown_signal()));

    let api_prefix = cfg.http.api_prefix;
    let uri = "/authority";

    let client = Client::new();
    // build body
    let body = AuthorityInsertReq {
        name: "test_authority".to_string(),
        authority_uri: "test.authority.path".to_string(),
        method: "GET".to_string(),
    };

    let resp = client
        .post(format!("http://{}{}{}", addr, api_prefix, uri))
        .json(&body)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    let authority: AuthorityVO = resp.json().await?;
    assert_eq!(authority.name, "test_authority");
    assert_eq!(authority.authority_uri, "test.authority.path");

    Ok(())
}

#[tokio::test]
async fn test_get_page_authorities() -> Result<(), AppError> {
    // 运行一个docker镜像
    let (_pg, db_url) = get_postgres_container().await?;

    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    let token = get_test_token(&mut cfg);
    tokio::spawn(cobotusr::serve(cfg.clone(), listener, shutdown_signal()));

    let api_prefix = cfg.http.api_prefix;
    let uri = "/authority?page_number=1&page_size=10";

    let client = Client::new();
    let resp = client
        .get(format!("http://{}{}{}", addr, api_prefix, uri))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    Ok(())
}

#[tokio::test]
async fn test_patch_authority() -> Result<(), AppError> {
    // 运行一个docker镜像
    let (_pg, db_url) = get_postgres_container().await?;

    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    let token = get_test_token(&mut cfg);
    tokio::spawn(cobotusr::serve(cfg.clone(), listener, shutdown_signal()));

    let api_prefix = cfg.http.api_prefix;
    let uri = "/authority";

    let client = Client::new();
    // 先创建一个权限
    let create_body = AuthorityInsertReq {
        name: "patch_test_authority".to_string(),
        authority_uri: "patch.test.authority".to_string(),
        method: "patch".to_string(),
    };

    let resp = client
        .post(format!("http://{}{}{}", addr, api_prefix, uri))
        .header("Authorization", format!("Bearer {}", token))
        .json(&create_body)
        .send()
        .await?
        .error_for_status()?;

    let authority: AuthorityVO = resp.json().await?;

    // 然后修改这个权限
    let patch_body = AuthorityPatchReq {
        id: authority.id,
        name: Some("patched_authority".to_string()),
        authority_uri: Some("/patch_test_authority".to_string()),
        method: Some("PATCH".to_string()),
    };

    let resp = client
        .patch(format!("http://{}{}{}", addr, api_prefix, uri))
        .header("Authorization", format!("Bearer {}", token))
        .json(&patch_body)
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    Ok(())
}

#[tokio::test]
async fn test_delete_authority() -> Result<(), AppError> {
    // 运行一个docker镜像
    let (_pg, db_url) = get_postgres_container().await?;

    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    let token = get_test_token(&mut cfg);
    tokio::spawn(cobotusr::serve(cfg.clone(), listener, shutdown_signal()));

    let api_prefix = cfg.http.api_prefix;
    let uri = "/authority";

    let client = Client::new();
    // 先创建一个权限
    let create_body = AuthorityInsertReq {
        name: "delete_test_authority".to_string(),
        authority_uri: "delete.test.authority".to_string(),
        method: "DELETE".to_string(),
    };

    let resp = client
        .post(format!("http://{}{}{}", addr, api_prefix, uri))
        .json(&create_body)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    let authority: AuthorityVO = resp.json().await?;

    // 然后删除这个权限
    let delete_body = AuthorityDeleteReq { id: authority.id };

    let resp = client
        .delete(format!("http://{}{}{}", addr, api_prefix, uri))
        .header("Authorization", format!("Bearer {}", token))
        .json(&delete_body)
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    Ok(())
}
