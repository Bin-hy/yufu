use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::Client;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use serde_json::{Value, json};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use yufu::config::Config;
use yufu::error::AppError;

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
    panic!("yufu did not become ready");
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
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let mut cfg = Config::default();
    cfg.database.url = db_url.clone();
    cfg.oidc.issuer = format!("http://{addr}");
    cfg.auth.secret = "mcp-http-test-key".into();
    cfg.auth.exp_seconds = 300;
    cfg.auth.refresh_exp_seconds = 3600;
    let (_legacy, _rsa) = super::oidc::rsa_test_token(&mut cfg);
    let api = cfg.http.api_prefix.clone();
    let clock = std::sync::Arc::new(auth::security::budget::SystemClock::new(
        std::sync::Arc::new(|| true),
    ));
    tokio::spawn(yufu::serve_with_security_clock(
        cfg,
        listener,
        shutdown_signal(),
        clock,
    ));
    wait_ready(&addr).await;
    let db = Database::connect(&db_url).await?;
    seed_resource_application(&db).await;
    let selector = serde_json::to_string(
        &auth::security::policy::ResourceSelector::new(
            "resource-app",
            auth::security::policy::ResourceScope::Application {
                kind: auth::security::policy::ResourceKind::Application,
            },
        )
        .unwrap(),
    )
    .unwrap();
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "UPDATE permission SET action='mcp:access',resource_type='application',resource_pattern=$1 WHERE name='mcp:access' AND application_id=(SELECT id FROM application WHERE app_key='resource-app')",[selector.into()])).await?;
    db.execute_raw(Statement::from_string(DbBackend::Postgres,
        r#"UPDATE "user" SET username='mcp-http-manager',password='$2y$04$FAiHSb5/5zp.oDR1GjBYSex5RVBJYW1BdEVBwqpdg4Phfk1i7YIPu' WHERE id=1"#.to_owned())).await?;
    let token = login(&addr, &api).await;
    Ok((pg, addr, api, token, db_url))
}
async fn with_csrf(addr: &SocketAddr, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    let response = Client::new()
        .get(format!("http://{addr}/api/v1/csrf"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let body: Value = response.json().await.unwrap();
    request
        .header("origin", format!("http://{addr}"))
        .header("cookie", cookie)
        .header("x-csrf-token", body["csrf_token"].as_str().unwrap())
}
async fn login(addr: &SocketAddr, api: &str) -> String {
    let body: Value = with_csrf(
        addr,
        Client::new().post(format!("http://{addr}{api}/login")),
    )
    .await
    .json(&json!({"username":"mcp-http-manager","password":"test password"}))
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap();
    body["access_token"].as_str().unwrap().into()
}
async fn create_user_and_token(addr: &SocketAddr, api: &str, username: &str) -> String {
    let body: Value = with_csrf(
        addr,
        Client::new().post(format!("http://{addr}{api}/register")),
    )
    .await
    .json(&json!({"username":username,"nickname":username,"password":"test password"}))
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap();
    body["access_token"].as_str().unwrap().into()
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
        .json(&json!({ "aud": "resource-app" }))
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

// 登记管理会话和目标应用结构化上界 → 200，TTL 被夹到上限
#[tokio::test]
async fn mcp_token_success_and_ttl_clamped() -> Result<(), AppError> {
    let (_pg, addr, api, token, _db_url) = start().await?;

    // Request 9999 days — must be clamped to the default max (90).
    let resp = Client::new()
        .post(format!("http://{}{}/oidc/mcp-tokens", addr, api))
        .bearer_auth(&token)
        .json(&json!({ "aud": "resource-app", "ttl_days": 9999 }))
        .send()
        .await?;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: Value = resp.json().await?;
    assert_eq!(body["aud"], "resource-app");
    let access_token = body["access_token"].as_str().expect("access_token");
    let claims = decode_claims(access_token);
    assert_eq!(claims["aud"], "resource-app");
    assert_eq!(claims["token_use"], "mcp_access");
    assert_eq!(claims["principal_kind"], "user");
    assert!(
        claims["bounds"]["grants"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["operation"] == "mcp:access")
    );
    assert!(claims.get("permissions").is_none());

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
    _db_url: &str,
    addr: &SocketAddr,
    api: &str,
    token: &str,
    aud: &str,
    name: &str,
) -> (reqwest::Client, Value, String) {
    let client = Client::new();
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
        seed_and_mint(&db_url, &addr, &api, &token, "resource-app", "我的令牌").await;

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

// 用户 B 不可见/不可操作用户 A 的令牌：list 不含，变更返回403/404，
// 且 A 的令牌保持可用。
#[tokio::test]
async fn mcp_token_cross_user_isolation() -> Result<(), AppError> {
    let (_pg, addr, api, token_a, db_url) = start().await?;
    let (client, body, _jti) =
        seed_and_mint(&db_url, &addr, &api, &token_a, "resource-app", "甲的令牌").await;
    let id_a = body["id"].as_i64().unwrap();

    let token_b = create_user_and_token(&addr, &api, "mcp-iso-user-b").await;

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
        seed_and_mint(&db_url, &addr, &api, &token, "resource-app", "原名").await;

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
        seed_and_mint(&db_url, &addr, &api, &token, "resource-app", "轮换令牌").await;

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
        seed_and_mint(&db_url, &addr, &api, &token, "resource-app", "待删除").await;

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
    // Compatibility endpoint only. Full service authentication and registry
    // introspection migrate in later tasks; this is not evidence of liveness.
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

#[tokio::test]
async fn security_t26_http_management_rejects_legacy_unknown_business_and_mcp_bearers()
-> Result<(), AppError> {
    let (_pg, addr, api, manager, _db) = start().await?;
    let client = Client::new();
    let mut cfg = Config::default();
    cfg.oidc.issuer = format!("http://{addr}");
    let (legacy, _rsa) = super::oidc::rsa_test_token(&mut cfg);
    let signer = auth::token_manager::IdTokenSigner::from_rsa_pem(
        &std::fs::read(&cfg.oidc.rsa_private_key_path).unwrap(),
        300,
    )
    .unwrap();
    let original: auth::security::credential::SecurityAccessClaims =
        serde_json::from_value(decode_claims(&manager)).unwrap();
    let mut unknown = original.clone();
    unknown.jti = uuid::Uuid::new_v4().to_string();
    let mut business = original.clone();
    business.aud = "resource-app".into();
    business.application = "resource-app".into();
    let mut expired = original;
    expired.exp = 1;
    let mut bad = vec![
        legacy,
        signer
            .sign_security_access(&unknown)
            .unwrap()
            .expose_secret()
            .to_owned(),
        signer
            .sign_security_access(&business)
            .unwrap()
            .expose_secret()
            .to_owned(),
        signer
            .sign_security_access(&expired)
            .unwrap()
            .expose_secret()
            .to_owned(),
    ];
    let (_, body, _) = seed_and_mint("", &addr, &api, &manager, "resource-app", "strict").await;
    bad.push(body["access_token"].as_str().unwrap().into());
    let id = body["id"].as_i64().unwrap();
    for token in bad {
        for (method, path, payload) in [
            (
                "POST",
                "/oidc/mcp-tokens".to_string(),
                json!({"aud":"resource-app"}),
            ),
            ("GET", "/oidc/mcp-tokens".to_string(), json!({})),
            (
                "PATCH",
                format!("/oidc/mcp-tokens/{id}"),
                json!({"name":"bad"}),
            ),
            ("DELETE", format!("/oidc/mcp-tokens/{id}"), json!({})),
            ("POST", format!("/oidc/mcp-tokens/{id}/refresh"), json!({})),
        ] {
            let response = client
                .request(method.parse().unwrap(), format!("http://{addr}{api}{path}"))
                .bearer_auth(&token)
                .json(&payload)
                .send()
                .await?;
            assert_eq!(
                response.status(),
                401,
                "{method} {path} must require a registered interactive manager"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn security_t26_http_cookie_mutations_require_real_csrf_and_origin() -> Result<(), AppError> {
    let (_pg, addr, api, manager, _db) = start().await?;
    let client = Client::new();
    let url = format!("http://{addr}{api}/oidc/mcp-tokens");
    let response = client
        .post(&url)
        .bearer_auth(&manager)
        .header("cookie", "refresh_token=placeholder")
        .json(&json!({"aud":"resource-app"}))
        .send()
        .await?;
    assert_eq!(response.status(), 403);
    let response = with_csrf(&addr, client.post(&url).bearer_auth(&manager))
        .await
        .header("origin", "https://other.test")
        .json(&json!({"aud":"resource-app"}))
        .send()
        .await?;
    assert_eq!(response.status(), 403);
    let response = with_csrf(&addr, client.post(&url).bearer_auth(&manager))
        .await
        .json(&json!({"aud":"resource-app"}))
        .send()
        .await?;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let set: Value = response.json().await?;
    let id = set["id"].as_i64().unwrap();
    for (method, path, body) in [
        (
            "PATCH",
            format!("/oidc/mcp-tokens/{id}"),
            json!({"name":"renamed"}),
        ),
        ("POST", format!("/oidc/mcp-tokens/{id}/refresh"), json!({})),
        ("DELETE", format!("/oidc/mcp-tokens/{id}"), json!({})),
    ] {
        let response = client
            .request(method.parse().unwrap(), format!("http://{addr}{api}{path}"))
            .bearer_auth(&manager)
            .header("cookie", "other=value")
            .json(&body)
            .send()
            .await?;
        assert_eq!(response.status(), 403);
    }
    Ok(())
}

#[tokio::test]
async fn security_t27_http_parallel_rotation_and_unknown_legacy_metadata_are_closed()
-> Result<(), AppError> {
    let (_pg, addr, api, manager, db_url) = start().await?;
    let (client, old, _) =
        seed_and_mint(&db_url, &addr, &api, &manager, "resource-app", "parallel").await;
    let id = old["id"].as_i64().unwrap();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(8));
    let mut jobs = Vec::new();
    for _ in 0..8 {
        let (client, manager, barrier) = (client.clone(), manager.clone(), barrier.clone());
        let url = format!("http://{addr}{api}/oidc/mcp-tokens/{id}/refresh");
        jobs.push(tokio::spawn(async move {
            barrier.wait().await;
            client
                .post(url)
                .bearer_auth(manager)
                .json(&json!({}))
                .send()
                .await
                .unwrap()
        }));
    }
    let mut success = 0;
    for job in jobs {
        let response = job.await.unwrap();
        if response.status() == 200 {
            success += 1;
        } else {
            assert_eq!(response.status(), 409);
        }
    }
    assert_eq!(success, 1);
    let db = Database::connect(db_url).await?;
    db.execute_raw(Statement::from_string(DbBackend::Postgres,"INSERT INTO mcp_token (user_id,name,jti,aud,created_at,expires_at,revoked) VALUES (1,'legacy','not-registered','resource-app',NOW(),NOW()+INTERVAL '1 day',FALSE)".to_owned())).await?;
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT id FROM mcp_token WHERE jti='not-registered'".to_owned(),
        ))
        .await?
        .unwrap();
    let legacy: i32 = row.try_get("", "id")?;
    for (method, path) in [
        ("POST", format!("/oidc/mcp-tokens/{legacy}/refresh")),
        ("DELETE", format!("/oidc/mcp-tokens/{legacy}")),
    ] {
        let response = client
            .request(method.parse().unwrap(), format!("http://{addr}{api}{path}"))
            .bearer_auth(&manager)
            .json(&json!({}))
            .send()
            .await?;
        assert_eq!(response.status(), 403);
    }
    let list: Vec<Value> = client
        .get(format!("http://{addr}{api}/oidc/mcp-tokens"))
        .bearer_auth(&manager)
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(list.len(), 1);
    assert!(
        list.iter()
            .all(|row| row.get("access_token").is_none() && row["name"] != "legacy")
    );
    Ok(())
}

#[tokio::test]
async fn auth_center_mcp_requires_explicit_registered_audience() -> Result<(), AppError> {
    let (_pg, addr, api, token, _db_url) = start().await?;
    for (body, expected) in [
        (json!({}), 400),
        (json!({"aud":""}), 400),
        (json!({"aud":"unregistered-app"}), 403),
    ] {
        let request = Client::new()
            .post(format!("http://{addr}{api}/oidc/mcp-tokens"))
            .bearer_auth(&token)
            .json(&body);
        let response = with_csrf(&addr, request).await.send().await?;
        assert_eq!(response.status().as_u16(), expected);
    }
    Ok(())
}
