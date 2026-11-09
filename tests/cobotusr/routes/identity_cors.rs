use super::{
    super::common::IdentityFixture,
    identity_support::*,
    identity_token::{VERIFIER, code},
};
use cobotusr::domain::identity::application::ClientKind;
use sea_orm::ConnectionTrait;

#[tokio::test]
async fn identity_protocol_discovery_matches_current_implemented_capabilities_and_fixed_issuer() {
    let f = IdentityFixture::start().await;
    let response = f
        .client
        .get(format!("{}/.well-known/openid-configuration", f.base_url))
        .header("host", "forged.example")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let metadata = response.json::<serde_json::Value>().await.unwrap();
    assert_eq!(metadata["issuer"], f.base_url);
    assert_eq!(
        metadata["scopes_supported"],
        serde_json::json!(["openid", "profile", "email"])
    );
    assert_eq!(
        metadata["grant_types_supported"],
        serde_json::json!(["authorization_code", "refresh_token"])
    );
    assert_eq!(
        metadata["code_challenge_methods_supported"],
        serde_json::json!(["S256"])
    );
    assert_eq!(
        metadata["response_modes_supported"],
        serde_json::json!(["query"])
    );
    assert_eq!(
        metadata["end_session_endpoint"],
        format!("{}/oidc/end-session", f.base_url)
    );
    for (key, path) in [
        ("authorization_endpoint", "/oidc/authorize"),
        ("token_endpoint", "/oidc/token"),
        ("userinfo_endpoint", "/oidc/userinfo"),
        ("introspection_endpoint", "/oidc/introspect"),
        ("revocation_endpoint", "/oidc/revoke"),
        ("jwks_uri", "/.well-known/jwks.json"),
    ] {
        assert_eq!(metadata[key], format!("{}{path}", f.base_url));
    }
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_cors_requires_approved_spa_origin_and_never_opens_center_or_introspection()
 {
    let f = IdentityFixture::start().await;
    let a = app_kind(&f, "cors-spa", false, ClientKind::BrowserSpa).await;
    app(&f, "cors-web", false).await;
    let (_, root) = center_secret(&f, a.owner).await;
    for (path, method) in [
        ("/.well-known/openid-configuration", "GET"),
        ("/.well-known/jwks.json", "GET"),
        ("/oidc/token", "POST"),
        ("/oidc/userinfo", "GET"),
        ("/api/v1/oidc/userinfo", "POST"),
    ] {
        let response = f
            .client
            .request(reqwest::Method::OPTIONS, format!("{}{path}", f.base_url))
            .header("origin", "https://rp.example")
            .header("access-control-request-method", method)
            .header(
                "access-control-request-headers",
                "authorization, content-type",
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 204, "{path}");
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "https://rp.example"
        );
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-credentials")
        );
        assert!(!response.headers().contains_key("set-cookie"));
        for origin in [
            "https://evil.example",
            "https://rp.example.evil.test",
            "https://rp.example/",
        ] {
            let response = f
                .client
                .request(reqwest::Method::OPTIONS, format!("{}{path}", f.base_url))
                .header("origin", origin)
                .header("access-control-request-method", method)
                .send()
                .await
                .unwrap();
            assert!(response.status().is_client_error());
            assert!(
                !response
                    .headers()
                    .contains_key("access-control-allow-origin")
            );
        }
    }
    for path in [
        "/oidc/introspect",
        "/oidc/revoke",
        "/oidc/logout",
        "/oidc/authorize",
        "/api/v1/identity/csrf",
        "/api/v1/account/session",
        "/api/v1/admin/accounts",
    ] {
        let response = f
            .client
            .request(reqwest::Method::OPTIONS, format!("{}{path}", f.base_url))
            .header("origin", "https://rp.example")
            .header("access-control-request-method", "POST")
            .send()
            .await
            .unwrap();
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-origin"),
            "{path}"
        );
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-credentials")
        );
    }
    let web_code = code(&f, "cors-web-dev", &root, "openid").await;
    let response = f
        .client
        .post(format!("{}/oidc/token", f.base_url))
        .header("origin", "https://rp.example")
        .basic_auth("cors-web-dev", Some("fixture-client-secret"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", web_code.expose_secret()),
            ("redirect_uri", "https://rp.example/callback"),
            ("code_verifier", VERIFIER),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["error"],
        "invalid_request"
    );
    let spa_code = code(&f, "cors-spa-dev", &root, "openid profile").await;
    let response = f
        .client
        .post(format!("{}/oidc/token", f.base_url))
        .header("origin", "https://rp.example")
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", "cors-spa-dev"),
            ("code", spa_code.expose_secret()),
            ("redirect_uri", "https://rp.example/callback"),
            ("code_verifier", VERIFIER),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers()["access-control-allow-origin"],
        "https://rp.example"
    );
    assert!(
        !response
            .headers()
            .contains_key("access-control-allow-credentials")
    );
    assert!(!response.headers().contains_key("set-cookie"));
    let tokens = response.json::<serde_json::Value>().await.unwrap();
    let response = f
        .client
        .get(format!("{}/oidc/userinfo", f.base_url))
        .header("origin", "https://rp.example")
        .bearer_auth(tokens["access_token"].as_str().unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers()["access-control-allow-origin"],
        "https://rp.example"
    );
    f.db.execute_unprepared(
        "UPDATE security_session_client SET enabled=FALSE WHERE client_id='cors-spa-dev'",
    )
    .await
    .unwrap();
    let response = f
        .client
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/oidc/token", f.base_url),
        )
        .header("origin", "https://rp.example")
        .header("access-control-request-method", "POST")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    assert!(
        !response
            .headers()
            .contains_key("access-control-allow-origin")
    );
    f.close().await;
}
