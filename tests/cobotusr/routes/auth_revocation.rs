use super::*;
use cobotusr::service::oidc::{CreateAuthCodeParams, OidcService};
use sea_orm::EntityTrait;

struct BrowserSession {
    access: String,
    refresh_cookie: String,
}
async fn register_session(fixture: &LocalHttpFixture) -> BrowserSession {
    let response = csrf_request(
        fixture,
        fixture.client.post(format!("{}/register", fixture.api)),
        None,
    )
    .json(&RegistryData {
        username: format!("t25-{}", uuid::Uuid::new_v4()),
        nickname: "revocation".into(),
        password: "test password".into(),
    })
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), 200);
    let refresh_cookie = response_refresh_cookie(&response);
    let body: serde_json::Value = response.json().await.unwrap();
    BrowserSession {
        access: body["access_token"].as_str().unwrap().into(),
        refresh_cookie,
    }
}
async fn manager_session(fixture: &LocalHttpFixture) -> BrowserSession {
    fixture.db.execute_raw(Statement::from_string(DbBackend::Postgres,"UPDATE \"user\" SET username='t25-manager',password='$2y$04$FAiHSb5/5zp.oDR1GjBYSex5RVBJYW1BdEVBwqpdg4Phfk1i7YIPu' WHERE id=1".to_owned())).await.unwrap();
    let response = csrf_request(
        fixture,
        fixture.client.post(format!("{}/login", fixture.api)),
        None,
    )
    .json(&LoginData {
        username: "t25-manager".into(),
        password: "test password".into(),
    })
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), 200);
    let refresh_cookie = response_refresh_cookie(&response);
    let body: serde_json::Value = response.json().await.unwrap();
    BrowserSession {
        access: body["access_token"].as_str().unwrap().into(),
        refresh_cookie,
    }
}
async fn oidc_pair(fixture: &LocalHttpFixture, client_id: &str) -> serde_json::Value {
    let redirect = format!("{}/callback", fixture.root);
    OidcService::register_client(
        &fixture.db,
        1,
        client_id.into(),
        client_id.into(),
        vec![redirect.clone()],
        vec!["openid".into()],
        vec!["authorization_code".into(), "refresh_token".into()],
        true,
    )
    .await
    .unwrap();
    fixture.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO security_session_client (issuer,client_id,audience,application,enabled) VALUES ($1,$2,'cobotsrv','cobotsrv',TRUE)",[fixture.root.clone().into(),client_id.into()])).await.unwrap();
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".to_owned();
    let code = OidcService::create_authorization_code(
        &fixture.db,
        CreateAuthCodeParams {
            user_id: 1,
            client_id: client_id.into(),
            redirect_uri: Some(redirect.clone()),
            code_challenge: Some(challenge),
            code_challenge_method: "S256".into(),
            scope: Some("openid".into()),
            state: None,
            nonce: None,
        },
    )
    .await
    .unwrap();
    fixture
        .client
        .post(format!("{}/oidc/token", fixture.root))
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("redirect_uri", redirect.as_str()),
            ("code", code.as_str()),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}
async fn count(fixture: &LocalHttpFixture, sql: &str) -> i64 {
    fixture
        .db
        .query_one_raw(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "count")
        .unwrap()
}

#[tokio::test]
async fn security_t25_http_csrf_missing_untrusted_duplicate_and_wrong_binding_have_no_side_effects()
{
    let fixture = local_http_fixture("healthy").await;
    let session = register_session(&fixture).await;
    for mode in [
        "missing-origin",
        "untrusted-origin",
        "null-origin",
        "missing-proof",
        "wrong-binding",
        "duplicate-cookie",
        "duplicate-cookie-header",
        "duplicate-origin",
        "duplicate-proof",
    ] {
        let mut request = fixture.client.post(format!("{}/refresh", fixture.api));
        let proof = auth::security::csrf::CsrfProtector::new("t24-local-test-key".into())
            .unwrap()
            .issue(
                &fixture.root,
                if mode == "wrong-binding" {
                    None
                } else {
                    Some(
                        session
                            .refresh_cookie
                            .strip_prefix("refresh_token=")
                            .unwrap(),
                    )
                },
                fixture.clock.as_ref(),
            )
            .unwrap();
        if mode != "missing-origin" {
            request = request.header(
                "origin",
                match mode {
                    "untrusted-origin" => "http://attacker.invalid",
                    "null-origin" => "null",
                    _ => &fixture.root,
                },
            );
        }
        if mode == "duplicate-origin" {
            request = request.header("origin", &fixture.root);
        }
        if mode != "missing-proof" {
            request = request.header("x-csrf-token", proof.expose_secret());
        }
        if mode == "duplicate-proof" {
            request = request.header("x-csrf-token", proof.expose_secret());
        }
        if mode == "duplicate-cookie-header" {
            request = request.header("cookie", &session.refresh_cookie);
        }
        let mut cookies = format!(
            "{}; __Host-cobotusr-csrf={}",
            session.refresh_cookie,
            proof.expose_secret()
        );
        if mode == "duplicate-cookie" {
            cookies.push_str(&format!("; {}", session.refresh_cookie));
        }
        let response = request.header("cookie", cookies).send().await.unwrap();
        assert_eq!(response.status(), 403, "{mode}");
        assert!(!response.headers().contains_key("set-cookie"), "{mode}");
    }
    assert_eq!(count(&fixture,"SELECT COUNT(*) AS count FROM security_credential WHERE consumed_at IS NOT NULL OR revoked_at IS NOT NULL").await,0);
    let unsafe_login = fixture
        .client
        .post(format!("{}/login", fixture.api))
        .json(&LoginData {
            username: "unknown".into(),
            password: "test password".into(),
        })
        .send()
        .await
        .unwrap();
    assert_eq!(unsafe_login.status(), 403);
    let safe = csrf_request(
        &fixture,
        fixture.client.post(format!("{}/refresh", fixture.api)),
        Some(&session.refresh_cookie),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(safe.status(), 200);
}

#[tokio::test]
async fn security_t25_http_logout_only_clears_cookies_after_committed_session_revocation() {
    let fixture = local_http_fixture("healthy").await;
    let session = register_session(&fixture).await;
    fixture.db.execute_raw(Statement::from_string(DbBackend::Postgres,"CREATE FUNCTION reject_t25_http() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test revoke rejected'; END $$".to_owned())).await.unwrap();
    fixture.db.execute_raw(Statement::from_string(DbBackend::Postgres,"CREATE CONSTRAINT TRIGGER reject_t25_http AFTER UPDATE ON security_session DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_t25_http()".to_owned())).await.unwrap();
    let rejected = csrf_request(
        &fixture,
        fixture.client.post(format!("{}/logout", fixture.api)),
        Some(&session.refresh_cookie),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(rejected.status(), 503);
    assert!(!rejected.headers().contains_key("set-cookie"));
    assert_eq!(
        count(
            &fixture,
            "SELECT COUNT(*) AS count FROM security_session WHERE revoked_at IS NOT NULL"
        )
        .await,
        0
    );
    fixture
        .db
        .execute_raw(Statement::from_string(
            DbBackend::Postgres,
            "DROP TRIGGER reject_t25_http ON security_session".to_owned(),
        ))
        .await
        .unwrap();
    let response = csrf_request(
        &fixture,
        fixture.client.post(format!("{}/logout", fixture.api)),
        Some(&session.refresh_cookie),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), 200);
    let cookies = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap())
        .collect::<Vec<_>>();
    assert!(
        cookies
            .iter()
            .any(|v| v.starts_with("refresh_token=;") && v.contains("Max-Age=0"))
    );
    let receipt: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        receipt["effective_by"].as_u64().unwrap() - receipt["committed_at"].as_u64().unwrap(),
        10_000
    );
    assert_eq!(
        count(
            &fixture,
            "SELECT COUNT(*) AS count FROM security_credential WHERE revoked_at IS NOT NULL"
        )
        .await,
        2
    );
    let refresh = csrf_request(
        &fixture,
        fixture.client.post(format!("{}/refresh", fixture.api)),
        Some(&session.refresh_cookie),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(refresh.status(), 401);
}

#[tokio::test]
async fn security_t25_http_oidc_no_hint_access_and_refresh_revocation_are_registered_and_client_scoped()
 {
    let fixture = local_http_fixture("healthy").await;
    let first = oidc_pair(&fixture, "t25-client").await;
    let other = oidc_pair(&fixture, "t25-other-client").await;
    let first_refresh = first["refresh_token"].as_str().unwrap();
    let wrong = fixture
        .client
        .post(format!("{}/oidc/revoke", fixture.root))
        .form(&[("token", first_refresh), ("client_id", "t25-other-client")])
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 200);
    assert_eq!(
        count(
            &fixture,
            "SELECT COUNT(*) AS count FROM security_credential WHERE revoked_at IS NOT NULL"
        )
        .await,
        0
    );
    let no_hint = fixture
        .client
        .post(format!("{}/oidc/revoke", fixture.root))
        .form(&[("token", first_refresh), ("client_id", "t25-client")])
        .send()
        .await
        .unwrap();
    assert_eq!(no_hint.status(), 200);
    let receipt: serde_json::Value = no_hint.json().await.unwrap();
    assert!(receipt["committed_at"].as_u64().is_some());
    let revoked = fixture
        .client
        .post(format!("{}/oidc/token", fixture.root))
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", "t25-client"),
            ("refresh_token", first_refresh),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(revoked.status(), 401);
    let access_revoke = fixture
        .client
        .post(format!("{}/oidc/revoke", fixture.root))
        .form(&[
            ("token", other["access_token"].as_str().unwrap()),
            ("token_type_hint", "unknown"),
            ("client_id", "t25-other-client"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(access_revoke.status(), 200);
    assert_eq!(count(&fixture,"SELECT COUNT(*) AS count FROM security_credential WHERE credential_kind='access' AND revoked_at IS NOT NULL").await,2);
}

#[tokio::test]
async fn security_t25_http_consent_revocation_commits_registry_roots_and_consent_together() {
    let fixture = local_http_fixture("healthy").await;
    let manager = manager_session(&fixture).await;
    oidc_pair(&fixture, "t25-consent-client").await;
    OidcService::save_consent(&fixture.db, 1, "t25-consent-client", "openid")
        .await
        .unwrap();
    let consent = cobotusr::entity::oauth_consent::Entity::find()
        .one(&fixture.db)
        .await
        .unwrap()
        .unwrap();
    let response = fixture
        .client
        .delete(format!(
            "{}/oidc/authorized-apps/{}",
            fixture.api, consent.id
        ))
        .bearer_auth(&manager.access)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let receipt: serde_json::Value = response.json().await.unwrap();
    assert!(receipt["effective_by"].as_u64().is_some());
    assert_eq!(count(&fixture,"SELECT COUNT(*) AS count FROM security_grant WHERE application='cobotsrv' AND revoked_at IS NOT NULL").await,1);
    assert_eq!(
        count(&fixture, "SELECT COUNT(*) AS count FROM oauth_consent").await,
        0
    );
    assert_eq!(count(&fixture,"SELECT COUNT(*) AS count FROM security_grant WHERE application='cobotusr' AND revoked_at IS NOT NULL").await,0);
}

#[tokio::test]
async fn security_t25_http_oidc_get_logout_is_confirmation_and_post_revokes_before_redirect() {
    let fixture = local_http_fixture("healthy").await;
    let session = register_session(&fixture).await;
    oidc_pair(&fixture, "t25-logout-client").await;
    let redirect = format!("{}/callback", fixture.root);
    let page = fixture
        .client
        .get(format!("{}/oidc/logout", fixture.root))
        .query(&[
            ("client_id", "t25-logout-client"),
            ("post_logout_redirect_uri", redirect.as_str()),
        ])
        .header("cookie", &session.refresh_cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(page.status(), 200);
    assert!(
        !page
            .headers()
            .get_all("set-cookie")
            .iter()
            .any(|v| v.to_str().unwrap().starts_with("refresh_token="))
    );
    let proof_cookie = page.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let body = page.text().await.unwrap();
    let proof = body
        .split("name=\"csrf_token\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(proof_cookie, format!("__Host-cobotusr-csrf={proof}"));
    assert!(body.contains("method=\"post\""));
    assert!(!body.contains("localStorage.removeItem"));
    assert_eq!(
        count(
            &fixture,
            "SELECT COUNT(*) AS count FROM security_session WHERE revoked_at IS NOT NULL"
        )
        .await,
        0
    );
    let invalid_redirect = fixture
        .client
        .post(format!("{}/oidc/logout", fixture.root))
        .header("origin", &fixture.root)
        .header(
            "cookie",
            format!("{}; {proof_cookie}", session.refresh_cookie),
        )
        .form(&[
            ("client_id", "t25-logout-client"),
            ("post_logout_redirect_uri", "http://attacker.invalid"),
            ("csrf_token", proof),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(invalid_redirect.status(), 400);
    assert!(!invalid_redirect.headers().contains_key("set-cookie"));
    assert_eq!(
        count(
            &fixture,
            "SELECT COUNT(*) AS count FROM security_session WHERE revoked_at IS NOT NULL"
        )
        .await,
        0
    );
    let response = fixture
        .client
        .post(format!("{}/oidc/logout", fixture.root))
        .header("origin", &fixture.root)
        .header(
            "cookie",
            format!("{}; {proof_cookie}", session.refresh_cookie),
        )
        .form(&[
            ("client_id", "t25-logout-client"),
            ("post_logout_redirect_uri", redirect.as_str()),
            ("csrf_token", proof),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.text().await.unwrap().contains("data-committed-at"));
    assert_eq!(
        count(
            &fixture,
            "SELECT COUNT(*) AS count FROM security_session WHERE revoked_at IS NOT NULL"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn security_t25_http_kickout_requires_registered_management_session_and_updates_new_registry()
{
    let fixture = local_http_fixture("healthy").await;
    let manager = manager_session(&fixture).await;
    let mut cfg = Config::default();
    let (legacy, _rsa) = super::super::oidc::rsa_test_token(&mut cfg);
    let denied = fixture
        .client
        .post(format!("{}/kickout", fixture.api))
        .bearer_auth(&legacy)
        .json(&serde_json::json!({"sub":1}))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 401);
    let cookie_denied = fixture
        .client
        .post(format!("{}/kickout", fixture.api))
        .bearer_auth(&manager.access)
        .header("cookie", &manager.refresh_cookie)
        .json(&serde_json::json!({"sub":1}))
        .send()
        .await
        .unwrap();
    assert_eq!(cookie_denied.status(), 403);
    let response = fixture
        .client
        .post(format!("{}/kickout", fixture.api))
        .bearer_auth(&manager.access)
        .json(&serde_json::json!({"sub":1}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let receipt: serde_json::Value = response.json().await.unwrap();
    assert!(receipt["effective_by"].as_u64().is_some());
    assert_eq!(
        count(
            &fixture,
            "SELECT COUNT(*) AS count FROM security_session WHERE revoked_at IS NOT NULL"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn security_t25_http_oidc_basic_auth_is_required_before_unknown_token_noop() {
    let fixture = local_http_fixture("healthy").await;
    let pair = oidc_pair(&fixture, "t25-basic-client").await;
    fixture
        .db
        .execute_raw(Statement::from_string(
            DbBackend::Postgres,
            "UPDATE oauth_client SET public_client=FALSE WHERE client_id='t25-basic-client'"
                .to_owned(),
        ))
        .await
        .unwrap();
    let client =
        cobotusr::entity::oauth_client::Entity::get_by_client_id(&fixture.db, "t25-basic-client")
            .await
            .unwrap()
            .unwrap();
    let (_, secret) = OidcService::regenerate_client_secret(&fixture.db, client.id, 1)
        .await
        .unwrap();
    let endpoint = format!("{}/oidc/revoke", fixture.root);
    let missing = fixture
        .client
        .post(&endpoint)
        .form(&[
            ("token", "unknown-value"),
            ("client_id", "t25-basic-client"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 401);
    let wrong = fixture
        .client
        .post(&endpoint)
        .basic_auth("t25-basic-client", Some("wrong"))
        .form(&[("token", "unknown-value")])
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 401);
    let conflict = fixture
        .client
        .post(&endpoint)
        .basic_auth("t25-basic-client", Some(secret.as_str()))
        .form(&[("token", "unknown-value"), ("client_id", "another-client")])
        .send()
        .await
        .unwrap();
    assert_eq!(conflict.status(), 400);
    let noop = fixture
        .client
        .post(&endpoint)
        .basic_auth("t25-basic-client", Some(secret.as_str()))
        .form(&[("token", "unknown-value")])
        .send()
        .await
        .unwrap();
    assert_eq!(noop.status(), 200);
    assert!(
        noop.json::<serde_json::Value>().await.unwrap()["committed_at"]
            .as_u64()
            .is_some()
    );
    assert_eq!(
        count(
            &fixture,
            "SELECT COUNT(*) AS count FROM security_credential WHERE revoked_at IS NOT NULL"
        )
        .await,
        0
    );
    let wrong_audience = fixture
        .client
        .post(format!("{}/kickout", fixture.api))
        .bearer_auth(pair["access_token"].as_str().unwrap())
        .json(&serde_json::json!({"sub":1}))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong_audience.status(), 401);
    let revoked = fixture
        .client
        .post(&endpoint)
        .basic_auth("t25-basic-client", Some(secret.as_str()))
        .form(&[
            ("token", pair["refresh_token"].as_str().unwrap()),
            ("token_type_hint", "access_token"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(revoked.status(), 200);
    assert_eq!(
        count(
            &fixture,
            "SELECT COUNT(*) AS count FROM security_credential WHERE revoked_at IS NOT NULL"
        )
        .await,
        2
    );
}

#[tokio::test]
async fn security_t25_http_bootstrap_and_rotated_proof_bind_to_current_cookie() {
    let fixture = local_http_fixture("healthy").await;
    let response = fixture
        .client
        .get(format!("{}/csrf", fixture.api))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        cookie.starts_with("__Host-cobotusr-csrf=")
            && cookie.contains("Secure")
            && cookie.contains("Path=/")
    );
    let body: serde_json::Value = response.json().await.unwrap();
    let registered = fixture
        .client
        .post(format!("{}/register", fixture.api))
        .header("origin", &fixture.root)
        .header("cookie", cookie.split(';').next().unwrap())
        .header("x-csrf-token", body["csrf_token"].as_str().unwrap())
        .json(&RegistryData {
            username: "bootstrap-user".into(),
            nickname: "bootstrap".into(),
            password: "test password".into(),
        })
        .send()
        .await
        .unwrap();
    assert_eq!(registered.status(), 200);
    let refresh = response_refresh_cookie(&registered);
    let session: serde_json::Value = registered.json().await.unwrap();
    let proof = session["csrf_token"].as_str().unwrap();
    let refreshed = fixture
        .client
        .post(format!("{}/refresh", fixture.api))
        .header("origin", &fixture.root)
        .header("cookie", format!("{refresh}; __Host-cobotusr-csrf={proof}"))
        .header("x-csrf-token", proof)
        .send()
        .await
        .unwrap();
    assert_eq!(refreshed.status(), 200);
    let rotated = response_refresh_cookie(&refreshed);
    let response: serde_json::Value = refreshed.json().await.unwrap();
    let stale = fixture
        .client
        .post(format!("{}/logout", fixture.api))
        .header("origin", &fixture.root)
        .header("cookie", format!("{rotated}; __Host-cobotusr-csrf={proof}"))
        .header("x-csrf-token", proof)
        .send()
        .await
        .unwrap();
    assert_eq!(stale.status(), 403);
    let proof = response["csrf_token"].as_str().unwrap();
    let logout = fixture
        .client
        .post(format!("{}/logout", fixture.api))
        .header("origin", &fixture.root)
        .header("cookie", format!("{rotated}; __Host-cobotusr-csrf={proof}"))
        .header("x-csrf-token", proof)
        .send()
        .await
        .unwrap();
    assert_eq!(logout.status(), 200);
}

#[tokio::test]
async fn security_t25_http_consent_commit_failure_preserves_consent_codes_and_roots() {
    let fixture = local_http_fixture("healthy").await;
    let manager = manager_session(&fixture).await;
    oidc_pair(&fixture, "t25-rollback-client").await;
    OidcService::save_consent(&fixture.db, 1, "t25-rollback-client", "openid")
        .await
        .unwrap();
    let consent = cobotusr::entity::oauth_consent::Entity::find()
        .one(&fixture.db)
        .await
        .unwrap()
        .unwrap();
    let code = OidcService::create_authorization_code(
        &fixture.db,
        CreateAuthCodeParams {
            user_id: 1,
            client_id: "t25-rollback-client".into(),
            redirect_uri: Some(format!("{}/callback", fixture.root)),
            code_challenge: Some("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".into()),
            code_challenge_method: "S256".into(),
            scope: Some("openid".into()),
            state: None,
            nonce: None,
        },
    )
    .await
    .unwrap();
    fixture.db.execute_raw(Statement::from_string(DbBackend::Postgres,"CREATE FUNCTION reject_consent_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test consent revoke rejected'; END $$".to_owned())).await.unwrap();
    fixture.db.execute_raw(Statement::from_string(DbBackend::Postgres,"CREATE CONSTRAINT TRIGGER reject_consent_commit AFTER UPDATE ON security_grant DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_consent_commit()".to_owned())).await.unwrap();
    let path = format!("{}/oidc/authorized-apps/{}", fixture.api, consent.id);
    let rejected = fixture
        .client
        .delete(&path)
        .bearer_auth(&manager.access)
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), 503);
    assert_eq!(
        count(&fixture, "SELECT COUNT(*) AS count FROM oauth_consent").await,
        1
    );
    assert_eq!(
        count(
            &fixture,
            "SELECT COUNT(*) AS count FROM oauth_authorization_code WHERE used=FALSE"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &fixture,
            "SELECT COUNT(*) AS count FROM security_grant WHERE revoked_at IS NOT NULL"
        )
        .await,
        0
    );
    fixture
        .db
        .execute_raw(Statement::from_string(
            DbBackend::Postgres,
            "DROP TRIGGER reject_consent_commit ON security_grant".to_owned(),
        ))
        .await
        .unwrap();
    assert_eq!(
        fixture
            .client
            .delete(&path)
            .bearer_auth(&manager.access)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        count(&fixture, "SELECT COUNT(*) AS count FROM oauth_consent").await,
        0
    );
    assert_eq!(
        count(
            &fixture,
            "SELECT COUNT(*) AS count FROM oauth_authorization_code WHERE used=FALSE"
        )
        .await,
        0
    );
    let cancelled = fixture
        .client
        .post(format!("{}/oidc/token", fixture.root))
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", "t25-rollback-client"),
            ("code", code.as_str()),
            (
                "redirect_uri",
                format!("{}/callback", fixture.root).as_str(),
            ),
            (
                "code_verifier",
                "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
            ),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(cancelled.status(), 400);
}
