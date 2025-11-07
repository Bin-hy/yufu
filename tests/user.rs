use cobotsrv::config::Config;
use cobotsrv::entity::role::RoleEnum;
use cobotsrv::entity::user::{UserAddWithRoleReq, UserPatchReq, UserVO};
use cobotsrv::entity::{PageSelectQuery, PageSelectQueryResult};
use cobotsrv::error::AppError;
use reqwest::Client;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;

mod common;
use common::*;

#[tokio::test]
async fn test_get_page_users() -> Result<(), AppError> {
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
    let uri = "/users";
    let query_page = PageSelectQuery::default();
    let client = Client::new();
    let resp = client
        .get(format!("http://{}{}{}", addr, api_prefix, uri))
        .query(&query_page)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    let result: PageSelectQueryResult<UserVO> = resp.json().await?;
    assert!(!result.data_list.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_patch_user() -> Result<(), AppError> {
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

    // 先注册一个用户
    let client = Client::new();

    // 修改用户信息
    let uri = "/user";
    let new_nickname = "更新后的测试用户";
    let patch_req = UserPatchReq {
        user_id: RoleEnum::Superadmin.default_id(),
        nickname: Some(new_nickname.to_string()),
        password: None,
    };

    let resp = client
        .patch(format!("http://{}{}{}", addr, api_prefix, uri))
        .json(&patch_req)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    let updated_user: UserVO = resp.json().await?;
    assert_eq!(updated_user.nickname, Some(new_nickname.to_string()));
    Ok(())
}

#[tokio::test]
async fn test_get_user_by_id() -> Result<(), AppError> {
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

    let client = Client::new();

    // 获取用户信息
    let uri = "/user".to_string();

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
async fn test_add_user_with_role() -> Result<(), AppError> {
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

    let client = Client::new();

    // 获取用户信息
    let uri = "/user".to_string();

    let test_user_name = uuid::Uuid::new_v4().to_string();
    let test_password = uuid::Uuid::new_v4().to_string();
    let payload = UserAddWithRoleReq {
        username: test_user_name.clone(),
        role_id: RoleEnum::Admin.default_id(),
        password: test_password.clone(),
        nickname: None,
    };
    let resp = client
        .post(format!("http://{}{}{}", addr, api_prefix, uri))
        .json(&payload)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());

    Ok(())
}
