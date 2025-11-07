use cobotsrv::config::Config;
use reqwest::Client;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;
mod common;
use cobotsrv::entity::PageSelectQueryResult;
use cobotsrv::entity::role::{RoleAddReq, RolePatchReq, RoleVO};
use cobotsrv::error::AppError;
use common::get_postgres_container;
use common::get_test_token;
use common::*;

#[tokio::test]
async fn test_add_role() -> Result<(), AppError> {
    // 运行一个docker镜像
    let (_pg, db_url) = get_postgres_container().await?;

    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    let token = get_test_token(cfg.clone().auth.secret);
    tokio::spawn(cobotsrv::serve(cfg.clone(), listener, shutdown_signal()));

    let api_prefix = cfg.http.api_prefix;
    let uri = "/role";

    let client = Client::new();
    let role_name = format!("test_role_{}", uuid::Uuid::new_v4());

    let role_req = RoleAddReq { name: role_name };

    let resp = client
        .post(format!("http://{}{}{}", addr, api_prefix, uri))
        .json(&role_req)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    Ok(())
}

#[tokio::test]
async fn test_get_page_role() -> Result<(), AppError> {
    // 运行一个docker镜像
    let (_pg, db_url) = get_postgres_container().await?;

    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    let token = get_test_token(cfg.clone().auth.secret);
    tokio::spawn(cobotsrv::serve(cfg.clone(), listener, shutdown_signal()));

    let api_prefix = cfg.http.api_prefix;
    let uri = "/role?page_number=1&page_size=10";

    let client = Client::new();
    let resp = client
        .get(format!("http://{}{}{}", addr, api_prefix, uri))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    let result: PageSelectQueryResult<RoleVO> = resp.json().await?;
    assert_eq!(result.page_number, 1);
    assert_eq!(result.page_size, 10);

    Ok(())
}

#[tokio::test]
async fn test_patch_role() -> Result<(), AppError> {
    // 运行一个docker镜像
    let (_pg, db_url) = get_postgres_container().await?;

    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;

    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    let token = get_test_token(cfg.clone().auth.secret);
    tokio::spawn(cobotsrv::serve(cfg.clone(), listener, shutdown_signal()));

    let api_prefix = cfg.http.api_prefix;
    let uri = "/role";

    let client = Client::new();
    // 先创建一个角色
    let role_name = format!("test_role_{}", uuid::Uuid::new_v4());

    let role_req = RoleAddReq { name: role_name };

    let resp = client
        .post(format!("http://{}{}{}", addr, api_prefix, uri))
        .header("Authorization", format!("Bearer {}", token))
        .json(&role_req)
        .send()
        .await?
        .error_for_status()?;

    let role: RoleVO = resp.json().await?;

    // 修改角色
    let new_name = "更新后的测试角色描述";
    let patch_req = RolePatchReq {
        id: role.id.clone(),
        name: new_name.to_string(),
    };

    let resp = client
        .patch(format!("http://{}{}{}", addr, api_prefix, uri))
        .header("Authorization", format!("Bearer {}", token))
        .json(&patch_req)
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    let updated_role: RoleVO = resp.json().await?;
    assert_eq!(updated_role.name, new_name.to_string());

    Ok(())
}
