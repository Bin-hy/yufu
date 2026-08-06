use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use cobotusr::config::Config;
use cobotusr::entity::role::RoleEnum;
use cobotusr::entity::{application, permission, user};
use cobotusr::error::AppError;
use reqwest::Client;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, Database};
use serde_json::{Value, json};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;

use super::super::common::*;

/// Poll a public endpoint until the server is serving (migrations done).
/// Uses OIDC discovery (always 200, regardless of signing alg — unlike JWKS,
/// which 404s in the test's HS256 mode).
async fn wait_ready(addr: &SocketAddr) {
    let url = format!("http://{}/.well-known/openid-configuration", addr);
    let c = Client::new();
    for _ in 0..100 {
        if c.get(&url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("cobotusr did not become ready");
}

/// Decode a JWT's payload (no verification) into JSON.
fn decode_claims(token: &str) -> Value {
    let payload = token.split('.').nth(1).expect("jwt has payload");
    let bytes = URL_SAFE_NO_PAD.decode(payload).expect("b64 decode");
    serde_json::from_slice(&bytes).expect("payload is json")
}

async fn start() -> Result<
    (
        testcontainers::ContainerAsync<testcontainers::GenericImage>,
        SocketAddr,
        String,
        String,
        String,
    ),
    AppError,
> {
    let (pg, db_url) = get_postgres_container().await?;
    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = TcpListener::bind(SocketAddr::new(ip, 0)).await?;
    let addr = listener.local_addr()?;
    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    let token = get_test_token(&mut cfg); // HS256 superadmin token (sub=1, aud=cobotusr)
    let api_prefix = cfg.http.api_prefix.clone();
    tokio::spawn(cobotusr::serve(cfg, listener, shutdown_signal()));
    wait_ready(&addr).await;
    Ok((pg, addr, api_prefix, token, db_url))
}

/// Insert a plain `User`-role account and mint an HS256 access token for it
/// (same signing mode as `get_test_token`, which forces HS256 on the server).
async fn create_user_and_token(db_url: &str, username: &str) -> (i32, String) {
    let db = Database::connect(db_url).await.unwrap();
    let now = chrono::Utc::now().naive_utc();
    let user = user::ActiveModel {
        id: NotSet,
        username: Set(username.to_string()),
        password: Set(None),
        nickname: Set(None),
        description: Set(None),
        role_id: Set(RoleEnum::User.default_id()),
        create_time: Set(now),
        update_time: Set(now),
    }
    .insert(&db)
    .await
    .unwrap();

    let cfg = Config::default();
    let signer = auth::token_manager::IdTokenSigner::from_hmac(&cfg.auth.secret, 3600);
    let token = signer
        .sign_access_token(
            &user.id.to_string(),
            "cobotusr-test",
            "cobotusr",
            "",
            3600,
            vec![],
            vec![],
        )
        .unwrap();
    (user.id, token)
}

// 无 mcp:access（目标应用不存在 → 权限为空）→ 403
#[tokio::test]
async fn mcp_token_requires_mcp_access() -> Result<(), AppError> {
    let (_pg, addr, api, token, _db) = start().await?;
    let resp = Client::new()
        .post(format!("http://{}{}/oidc/mcp-tokens", addr, api))
        .bearer_auth(&token)
        .json(&json!({ "aud": "no-such-app" }))
        .send()
        .await?;
    assert_eq!(resp.status(), reqwest::StatusCode::FORBIDDEN);
    Ok(())
}

// 未携带 bearer token → 401（签发与列表都必须在认证中间件之后）
#[tokio::test]
async fn mcp_token_unauthenticated_is_401() -> Result<(), AppError> {
    let (_pg, addr, api, _token, _db) = start().await?;
    let client = Client::new();

    let resp = client
        .post(format!("http://{}{}/oidc/mcp-tokens", addr, api))
        .json(&json!({ "aud": "cobotsrv" }))
        .send()
        .await?;
    assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);

    let resp = client
        .get(format!("http://{}{}/oidc/mcp-tokens", addr, api))
        .send()
        .await?;
    assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);
    Ok(())
}

// 应用持有 mcp:access（超管自动获全部权限）→ 200，令牌含正确 aud/permissions，TTL 被夹到上限
#[tokio::test]
async fn mcp_token_success_and_ttl_clamped() -> Result<(), AppError> {
    let (_pg, addr, api, token, db_url) = start().await?;

    // Seed an application carrying mcp:access for audience "mcp-test".
    let db = Database::connect(&db_url).await?;
    let now = chrono::Utc::now().naive_utc();
    let app = application::ActiveModel {
        id: NotSet,
        app_key: Set("mcp-test".to_string()),
        name: Set("MCP Test App".to_string()),
        description: Set(None),
        oidc_client_id: Set(None),
        create_time: Set(now),
        update_time: Set(now),
    }
    .insert(&db)
    .await?;
    permission::ActiveModel {
        id: NotSet,
        application_id: Set(app.id),
        name: Set("mcp:access".to_string()),
        description: Set(None),
        action: Set("access".to_string()),
        resource_type: Set(Some("mcp".to_string())),
        resource_pattern: Set(Some("*".to_string())),
        create_time: Set(now),
        update_time: Set(now),
    }
    .insert(&db)
    .await?;

    // Request 9999 days — must be clamped to the default max (90).
    let resp = Client::new()
        .post(format!("http://{}{}/oidc/mcp-tokens", addr, api))
        .bearer_auth(&token)
        .json(&json!({ "aud": "mcp-test", "ttl_days": 9999 }))
        .send()
        .await?;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: Value = resp.json().await?;
    assert_eq!(body["aud"], "mcp-test");
    let access_token = body["access_token"].as_str().expect("access_token");
    let claims = decode_claims(access_token);
    assert_eq!(claims["aud"], "mcp-test");
    assert!(
        claims["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == "mcp:access"),
        "token must carry mcp:access: {claims}"
    );

    // exp - iat clamped to 90 days.
    let exp = claims["exp"].as_i64().unwrap();
    let iat = claims["iat"].as_i64().unwrap();
    assert_eq!(exp - iat, 90 * 86_400);

    // New response fields (persisted token).
    assert!(
        body["id"].as_i64().is_some(),
        "must return persisted token id"
    );
    assert_eq!(body["name"], "未命名令牌");
    Ok(())
}

/// Quick helper: seed app + mcp:access permission, then create an MCP token.
async fn seed_and_mint(
    db_url: &str,
    addr: &SocketAddr,
    api: &str,
    token: &str,
    aud: &str,
    name: &str,
) -> (reqwest::Client, Value, String) {
    let client = Client::new();
    let db = Database::connect(db_url).await.unwrap();
    let now = chrono::Utc::now().naive_utc();
    let app = application::ActiveModel {
        id: NotSet,
        app_key: Set(aud.to_string()),
        name: Set(format!("App {aud}")),
        description: Set(None),
        oidc_client_id: Set(None),
        create_time: Set(now),
        update_time: Set(now),
    }
    .insert(&db)
    .await
    .unwrap();
    permission::ActiveModel {
        id: NotSet,
        application_id: Set(app.id),
        name: Set("mcp:access".to_string()),
        description: Set(None),
        action: Set("access".to_string()),
        resource_type: Set(Some("mcp".to_string())),
        resource_pattern: Set(Some("*".to_string())),
        create_time: Set(now),
        update_time: Set(now),
    }
    .insert(&db)
    .await
    .unwrap();

    let resp = client
        .post(format!("http://{}{}/oidc/mcp-tokens", addr, api))
        .bearer_auth(token)
        .json(&json!({ "aud": aud, "ttl_days": 7, "name": name }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::OK, "mint failed");
    let body: Value = resp.json().await.unwrap();
    let jti = decode_claims(body["access_token"].as_str().unwrap())["jti"]
        .as_str()
        .unwrap()
        .to_string();
    (client, body, jti)
}

// ── list ──

#[tokio::test]
async fn mcp_token_list_active_by_owner() -> Result<(), AppError> {
    let (_pg, addr, api, token, db_url) = start().await?;
    let (client, _body, _jti) =
        seed_and_mint(&db_url, &addr, &api, &token, "list-test", "我的令牌").await;

    let resp = client
        .get(format!("http://{}{}/oidc/mcp-tokens", addr, api))
        .bearer_auth(&token)
        .send()
        .await?;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let list: Vec<Value> = resp.json().await?;
    assert!(
        list.iter().any(|t| t["name"] == "我的令牌"),
        "list must contain the minted token"
    );
    Ok(())
}

// ── cross-user isolation ──

// 用户 B 不可见/不可操作用户 A 的令牌：list 不含、rename/refresh/delete 均 404，
// 且 A 的令牌保持可用。
#[tokio::test]
async fn mcp_token_cross_user_isolation() -> Result<(), AppError> {
    let (_pg, addr, api, token_a, db_url) = start().await?;
    let (client, body, _jti) =
        seed_and_mint(&db_url, &addr, &api, &token_a, "iso-test", "甲的令牌").await;
    let id_a = body["id"].as_i64().unwrap();

    let (_user_b_id, token_b) = create_user_and_token(&db_url, "mcp-iso-user-b").await;

    // B's list must not contain A's token.
    let resp = client
        .get(format!("http://{}{}/oidc/mcp-tokens", addr, api))
        .bearer_auth(&token_b)
        .send()
        .await?;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let list: Vec<Value> = resp.json().await?;
    assert!(
        !list.iter().any(|t| t["id"].as_i64() == Some(id_a)),
        "user B must not see user A's token: {list:?}"
    );

    // B cannot rename A's token (ownership-scoped → 404, not 403, to avoid id probing).
    let resp = client
        .patch(format!("http://{}{}/oidc/mcp-tokens/{id_a}", addr, api))
        .bearer_auth(&token_b)
        .json(&json!({ "name": "越权改名" }))
        .send()
        .await?;
    assert!(
        matches!(
            resp.status(),
            reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::NOT_FOUND
        ),
        "expected 403/404, got {}",
        resp.status()
    );

    // B cannot refresh A's token.
    let resp = client
        .post(format!(
            "http://{}{}/oidc/mcp-tokens/{id_a}/refresh",
            addr, api
        ))
        .bearer_auth(&token_b)
        .json(&json!({}))
        .send()
        .await?;
    assert!(
        matches!(
            resp.status(),
            reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::NOT_FOUND
        ),
        "expected 403/404, got {}",
        resp.status()
    );

    // B cannot delete A's token.
    let resp = client
        .delete(format!("http://{}{}/oidc/mcp-tokens/{id_a}", addr, api))
        .bearer_auth(&token_b)
        .send()
        .await?;
    assert!(
        matches!(
            resp.status(),
            reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::NOT_FOUND
        ),
        "expected 403/404, got {}",
        resp.status()
    );

    // A's token is untouched.
    let list: Vec<Value> = client
        .get(format!("http://{}{}/oidc/mcp-tokens", addr, api))
        .bearer_auth(&token_a)
        .send()
        .await?
        .json()
        .await?;
    assert!(
        list.iter().any(|t| t["name"] == "甲的令牌"),
        "user A's token must survive B's attempts"
    );
    Ok(())
}

// ── rename ──

#[tokio::test]
async fn mcp_token_rename_then_list() -> Result<(), AppError> {
    let (_pg, addr, api, token, db_url) = start().await?;
    let (client, body, _jti) =
        seed_and_mint(&db_url, &addr, &api, &token, "rename-test", "原名").await;

    let id = body["id"].as_i64().unwrap();
    let resp = client
        .patch(format!("http://{}{}/oidc/mcp-tokens/{id}", addr, api))
        .bearer_auth(&token)
        .json(&json!({ "name": "新名称" }))
        .send()
        .await?;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    // Verify via list.
    let list: Vec<Value> = client
        .get(format!("http://{}{}/oidc/mcp-tokens", addr, api))
        .bearer_auth(&token)
        .send()
        .await?
        .json()
        .await?;
    assert!(
        list.iter().any(|t| t["name"] == "新名称"),
        "name must be updated in list"
    );
    Ok(())
}

// ── refresh (rotate) ──

#[tokio::test]
async fn mcp_token_refresh_rotates_and_revokes_old() -> Result<(), AppError> {
    let (_pg, addr, api, token, db_url) = start().await?;
    let (client, body, old_jti) =
        seed_and_mint(&db_url, &addr, &api, &token, "rotate-test", "轮换令牌").await;

    let id = body["id"].as_i64().unwrap();
    let resp = client
        .post(format!(
            "http://{}{}/oidc/mcp-tokens/{id}/refresh",
            addr, api
        ))
        .bearer_auth(&token)
        .json(&json!({ "ttl_days": 7 }))
        .send()
        .await?;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let new_body: Value = resp.json().await?;
    let new_jti = decode_claims(new_body["access_token"].as_str().unwrap())["jti"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(new_jti, old_jti, "refresh must mint a new jti");

    // Introspect: old jti → revoked.
    let r: Value = client
        .post(format!("http://{}{}/oidc/mcp-tokens/introspect", addr, api))
        .bearer_auth(&token)
        .json(&json!({ "jti": old_jti }))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(r["revoked"], true, "old jti must be revoked after refresh");

    // New jti is not revoked.
    let r: Value = client
        .post(format!("http://{}{}/oidc/mcp-tokens/introspect", addr, api))
        .bearer_auth(&token)
        .json(&json!({ "jti": new_jti }))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(r["revoked"], false);

    Ok(())
}

// ── delete (revoke) ──

#[tokio::test]
async fn mcp_token_delete_revokes_and_hides_from_list() -> Result<(), AppError> {
    let (_pg, addr, api, token, db_url) = start().await?;
    let (client, body, jti) =
        seed_and_mint(&db_url, &addr, &api, &token, "delete-test", "待删除").await;

    let id = body["id"].as_i64().unwrap();
    let resp = client
        .delete(format!("http://{}{}/oidc/mcp-tokens/{id}", addr, api))
        .bearer_auth(&token)
        .send()
        .await?;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    // Gone from active list.
    let list: Vec<Value> = client
        .get(format!("http://{}{}/oidc/mcp-tokens", addr, api))
        .bearer_auth(&token)
        .send()
        .await?
        .json()
        .await?;
    assert!(
        !list.iter().any(|t| t["name"] == "待删除"),
        "deleted token must not appear in active list"
    );

    // Introspect reports revoked.
    let r: Value = client
        .post(format!("http://{}{}/oidc/mcp-tokens/introspect", addr, api))
        .bearer_auth(&token)
        .json(&json!({ "jti": jti }))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(r["revoked"], true);

    Ok(())
}

// ── introspect: unknown jti → false ──

#[tokio::test]
async fn mcp_token_introspect_unknown_is_not_revoked() -> Result<(), AppError> {
    // Deliberate fail-open semantics: an unknown jti is an ordinary OIDC session
    // token (never tracked in the mcp_token denylist), so it MUST report
    // `revoked: false` — only explicit revocations deny a jti. Do not "fix" this.
    let (_pg, addr, api, token, _db_url) = start().await?;
    let r: Value = Client::new()
        .post(format!("http://{}{}/oidc/mcp-tokens/introspect", addr, api))
        .bearer_auth(&token)
        .json(&json!({ "jti": "no-such-jti-00000000" }))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(r["revoked"], false);
    Ok(())
}
