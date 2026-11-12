use super::{
    super::common::IdentityFixture,
    identity_browser::{BrowserCookies, http_csrf},
    identity_support::*,
};
use axum::http::header::{CACHE_CONTROL, LOCATION, SET_COOKIE};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use uuid::Uuid;
use yufu::domain::identity::account::SecretDigest;

fn authorization_url(f: &IdentityFixture, client: &str, prompt: Option<&str>) -> reqwest::Url {
    let mut url = reqwest::Url::parse(&format!("{}/oidc/authorize", f.base_url)).unwrap();
    let mut query = url.query_pairs_mut();
    query.extend_pairs([
        ("client_id", client),
        ("redirect_uri", "https://rp.example/callback"),
        ("response_type", "code"),
        ("scope", "openid profile"),
        ("state", "state & +"),
        ("nonce", "http-request-nonce"),
        (
            "code_challenge",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ),
        ("code_challenge_method", "S256"),
    ]);
    if let Some(prompt) = prompt {
        query.append_pair("prompt", prompt);
    }
    drop(query);
    url
}
async fn begin(f: &IdentityFixture, cookies: &mut BrowserCookies) -> Uuid {
    begin_client(f, cookies, "http-auth-dev").await
}
async fn begin_client(f: &IdentityFixture, cookies: &mut BrowserCookies, client: &str) -> Uuid {
    let mut request = f.client.get(authorization_url(f, client, None));
    if !cookies.0.is_empty() {
        request = request.header("cookie", cookies.header());
    }
    let response = request.send().await.unwrap();
    if response.status() != 303 {
        let body = response.text().await.unwrap();
        panic!("authorization begin failed with {}", body);
    }
    assert_eq!(response.headers()[CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
    let uri = reqwest::Url::parse(&format!(
        "{}{}",
        f.base_url,
        response.headers()[LOCATION].to_str().unwrap()
    ))
    .unwrap();
    let pairs = uri.query_pairs().collect::<Vec<_>>();
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].0, "flow_id");
    let id = Uuid::parse_str(&pairs[0].1).unwrap();
    let proof_cookie = response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .map(|h| h.to_str().unwrap())
        .find(|s| s.starts_with(&format!("yufu-dev-flow-{id}=")))
        .unwrap();
    for attribute in ["HttpOnly", "Path=/", "SameSite=Lax", "Max-Age=600"] {
        assert!(proof_cookie.contains(attribute));
    }
    cookies.absorb(response.headers());
    id
}
async fn inspect(f: &IdentityFixture, cookies: &mut BrowserCookies, id: Uuid) -> serde_json::Value {
    let response = f
        .client
        .get(format!("{}/api/v1/identity/flows/{id}", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    cookies.absorb(response.headers());
    let value = response.json::<serde_json::Value>().await.unwrap();
    for key in [
        "proof",
        "state",
        "nonce",
        "code_challenge",
        "redirect_uri",
        "request_payload",
    ] {
        assert!(value.get(key).is_none());
    }
    value
}

#[tokio::test]
async fn identity_authorization_http_login_consent_and_recovery_use_only_flow_id() {
    let f = IdentityFixture::start().await;
    app(&f, "http-auth", true).await;
    let mut cookies = BrowserCookies::default();
    let id = begin(&f, &mut cookies).await;
    let summary = inspect(&f, &mut cookies, id).await;
    assert_eq!(summary["interaction"], "login");
    let one = f
        .client
        .get(format!("{}/api/v1/identity/flows/{id}", f.base_url))
        .header("cookie", cookies.header())
        .send();
    let two = f
        .client
        .get(format!("{}/api/v1/identity/flows/{id}", f.base_url))
        .header("cookie", cookies.header())
        .send();
    let (one, two) = tokio::join!(one, two);
    for response in [one.unwrap(), two.unwrap()] {
        assert_eq!(response.status(), 200);
        assert!(!response.headers().contains_key(SET_COOKIE));
    }
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_authorization_flow WHERE bound_sso_session_id IS NOT NULL OR completed_at IS NOT NULL").await,0);
    let response=f.client.post(format!("{}/api/v1/identity/login",f.base_url)).header("cookie",cookies.header()).header("origin",&f.base_url).header("x-csrf-token",summary["csrf_token"].as_str().unwrap()).json(&serde_json::json!({"username":"identity-admin","password":"fixture-admin-password","flow_id":id})).send().await.unwrap();
    assert_eq!(response.status(), 200);
    cookies.absorb(response.headers());
    let summary = inspect(&f, &mut cookies, id).await;
    assert_eq!(summary["interaction"], "consent");
    let endpoint = format!("{}/api/v1/identity/flows/{id}/complete", f.base_url);
    for (origin, csrf, payload) in [
        (None, None, serde_json::json!({"approved":true})),
        (
            Some(f.base_url.as_str()),
            None,
            serde_json::json!({"approved":true}),
        ),
        (
            Some("https://attacker.example"),
            Some(summary["csrf_token"].as_str().unwrap()),
            serde_json::json!({"approved":true}),
        ),
    ] {
        let mut request = f
            .client
            .post(&endpoint)
            .header("cookie", cookies.header())
            .json(&payload);
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        if let Some(csrf) = csrf {
            request = request.header("x-csrf-token", csrf);
        }
        assert_eq!(request.send().await.unwrap().status(), 403);
    }
    let response = f
        .client
        .post(&endpoint)
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", summary["csrf_token"].as_str().unwrap())
        .json(&serde_json::json!({"approved":true,"redirect_uri":"https://attacker.example"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    let response = f
        .client
        .post(&endpoint)
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", summary["csrf_token"].as_str().unwrap())
        .json(&serde_json::json!({"approved":true}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.headers().get_all(SET_COOKIE).iter().any(|h| {
        h.to_str()
            .unwrap()
            .starts_with(&format!("yufu-dev-flow-{id}=;"))
    }));
    cookies.absorb(response.headers());
    let receipt = response.json::<serde_json::Value>().await.unwrap();
    let uri = reqwest::Url::parse(receipt["redirect_url"].as_str().unwrap()).unwrap();
    let code = uri
        .query_pairs()
        .find(|(key, _)| key == "code")
        .unwrap()
        .1
        .into_owned();
    assert_eq!(
        uri.query_pairs().find(|(key, _)| key == "state").unwrap().1,
        "state & +"
    );
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM oauth_authorization_code WHERE code='sha256:{}' AND oidc_rp_session_id IS NOT NULL",SecretDigest::of(&code).hex())).await,1);
    let response = f
        .client
        .get(authorization_url(&f, "http-auth-dev", Some("none")))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 303);
    let uri = reqwest::Url::parse(response.headers()[LOCATION].to_str().unwrap()).unwrap();
    assert!(uri.query_pairs().any(|(key, _)| key == "code"));
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(DISTINCT protocol_sid)::bigint AS value FROM identity_oidc_rp_session"
        )
        .await,
        2
    );
    f.close().await;
}

#[tokio::test]
async fn identity_authorization_http_unassigned_user_receives_oauth_access_denied() {
    let f = IdentityFixture::start().await;
    app(&f, "http-deny", false).await;
    let password_hash = bcrypt::hash("fixture-unassigned-password", 4).unwrap();
    let row =
        f.db.query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO \"user\" (username,password,role_id,create_time,update_time)
             VALUES ($1,$2,1,NOW(),NOW()) RETURNING id",
            ["http-unassigned".into(), password_hash.into()],
        ))
        .await
        .unwrap();
    let unassigned_id: i32 = row.unwrap().try_get("", "id").unwrap();
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO identity_subject (issuer,sub,user_id,created_at)
             VALUES ($1,$2,$3,0)",
        [
            f.base_url.clone().into(),
            unassigned_id.to_string().into(),
            unassigned_id.into(),
        ],
    ))
    .await
    .unwrap();
    let mut cookies = BrowserCookies::default();
    let id = begin_client(&f, &mut cookies, "http-deny-dev").await;
    let login_summary = inspect(&f, &mut cookies, id).await;
    assert_eq!(login_summary["interaction"], "login");
    let response = f
        .client
        .post(format!("{}/api/v1/identity/login", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header(
            "x-csrf-token",
            login_summary["csrf_token"].as_str().unwrap(),
        )
        .json(&serde_json::json!({
            "username": "http-unassigned",
            "password": "fixture-unassigned-password",
            "flow_id": id
        }))
        .send()
        .await
        .unwrap();
    if response.status() != 200 {
        let body = response.text().await.unwrap();
        panic!("unassigned login failed with {}", body);
    }
    cookies.absorb(response.headers());
    let denied_summary = inspect(&f, &mut cookies, id).await;
    assert_eq!(denied_summary["interaction"], "denied");
    let response = f
        .client
        .post(format!(
            "{}/api/v1/identity/flows/{id}/complete",
            f.base_url
        ))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header(
            "x-csrf-token",
            denied_summary["csrf_token"].as_str().unwrap(),
        )
        .json(&serde_json::json!({"approved": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let receipt = response.json::<serde_json::Value>().await.unwrap();
    let uri = reqwest::Url::parse(receipt["redirect_url"].as_str().unwrap()).unwrap();
    assert_eq!(
        uri.query_pairs().find(|(key, _)| key == "error").unwrap().1,
        "access_denied"
    );
    assert_eq!(
        uri.query_pairs().find(|(key, _)| key == "state").unwrap().1,
        "state & +"
    );
    assert_eq!(
        scalar(
            &f,
            &format!(
                "SELECT COUNT(*)::bigint AS value FROM identity_authorization_flow
                 WHERE id='{id}' AND completed_at IS NOT NULL"
            )
        )
        .await,
        1
    );
    for table in [
        "oauth_authorization_code",
        "identity_oidc_rp_session",
        "oauth_consent",
    ] {
        assert_eq!(
            scalar(
                &f,
                &format!("SELECT COUNT(*)::bigint AS value FROM {table}")
            )
            .await,
            0
        );
    }
    f.close().await;
}

#[tokio::test]
async fn identity_authorization_http_errors_never_trust_unregistered_callbacks() {
    let f = IdentityFixture::start().await;
    app(&f, "http-auth", false).await;
    let mut cookies = BrowserCookies::default();
    http_csrf(&f, &mut cookies).await;
    let query = authorization_url(&f, "http-auth-dev", None);
    for url in [
        format!("{query}&client_id=http-auth-dev"),
        format!("{query}&%63lient_id=http-auth-dev"),
        format!("{query}&nonce=%ff"),
        format!("{query}&nonce=%"),
    ] {
        let response = f
            .client
            .get(url)
            .header("cookie", cookies.header())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        assert!(!response.headers().contains_key(LOCATION));
    }
    let mut uri = authorization_url(&f, "http-auth-dev", None);
    uri.set_query(Some(&uri.query().unwrap().replace(
        "https%3A%2F%2Frp.example%2Fcallback",
        "https%3A%2F%2Fattacker.example%2Fcallback",
    )));
    let response = f
        .client
        .get(uri)
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    assert!(!response.headers().contains_key(LOCATION));
    let response = f
        .client
        .get(authorization_url(&f, "unknown", None))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    assert!(!response.headers().contains_key(LOCATION));
    let response = f
        .client
        .get(authorization_url(&f, "http-auth-dev", Some("none")))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 303);
    let uri = reqwest::Url::parse(response.headers()[LOCATION].to_str().unwrap()).unwrap();
    assert_eq!(
        uri.query_pairs().find(|(key, _)| key == "error").unwrap().1,
        "login_required"
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_authorization_flow"
        )
        .await,
        0
    );
    let id = begin(&f, &mut cookies).await;
    let response = f
        .client
        .get(format!("{}/api/v1/identity/flows/{id}", f.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    f.db.execute_unprepared(
        "UPDATE application SET lifecycle='suspended' WHERE app_key='http-auth'",
    )
    .await
    .unwrap();
    let response = f
        .client
        .get(format!("{}/api/v1/identity/flows/{id}", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    f.close().await;
}
