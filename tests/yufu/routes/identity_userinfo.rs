use super::{
    super::common::IdentityFixture, identity_refresh::issued, identity_support::*,
    identity_token::protocol,
};
use auth::security::identity::SecretToken;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use yufu::{
    domain::identity::error::IdentityError, service::security::lifecycle::SessionTokenSigner,
};

#[tokio::test]
async fn identity_protocol_userinfo_discloses_only_the_token_scope_and_real_current_profile() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "userinfo-a", false).await;
    app(&f, "userinfo-b", false).await;
    let (_, root) = center_secret(&f, a.owner).await;
    let minimal = issued(&f, "userinfo-a-dev", &root, "openid").await;
    let full = issued(&f, "userinfo-b-dev", &root, "openid profile email").await;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET nickname=$2,email=$3,email_verified_at=NULL WHERE id=$1",
        [
            a.owner.into(),
            "Updated Name".into(),
            "actual@example.test".into(),
        ],
    ))
    .await
    .unwrap();
    let svc = protocol(&f, f.signer.clone());
    let profile = svc
        .userinfo(&minimal.access_token, f.signer.as_ref(), &budget(&f))
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(profile).unwrap(),
        serde_json::json!({"sub":a.owner.to_string()})
    );
    for path in ["/oidc/userinfo", "/api/v1/oidc/userinfo"] {
        for method in [reqwest::Method::GET, reqwest::Method::POST] {
            let response = f
                .client
                .request(method, format!("{}{path}", f.base_url))
                .bearer_auth(full.access_token.expose_secret())
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            assert_eq!(response.headers()["cache-control"], "no-store");
            assert_eq!(response.headers()["referrer-policy"], "no-referrer");
            assert!(!response.headers().contains_key("set-cookie"));
            assert_eq!(
                response.json::<serde_json::Value>().await.unwrap(),
                serde_json::json!({"sub":a.owner.to_string(),"name":"Updated Name","preferred_username":"identity-admin","email":"actual@example.test","email_verified":false})
            );
        }
    }
    f.db.execute_unprepared("UPDATE \"user\" SET email=NULL,email_verified_at=NULL")
        .await
        .unwrap();
    let profile = svc
        .userinfo(&full.access_token, f.signer.as_ref(), &budget(&f))
        .await
        .unwrap();
    let value = serde_json::to_value(profile).unwrap();
    assert!(value.get("email").is_none());
    assert!(value.get("email_verified").is_none());
    assert_eq!(value["sub"], a.owner.to_string());
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_userinfo_rejects_management_id_unknown_tampered_and_revoked_tokens() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "userinfo-invalid", false).await;
    let (root, root_secret) = center_secret(&f, a.owner).await;
    let set = issued(&f, "userinfo-invalid-dev", &root_secret, "openid profile").await;
    let manager = management_tokens(&f, &root).await;
    let mut fake = set.claims.clone();
    fake.jti = uuid::Uuid::new_v4().to_string();
    let unknown = f.signer.sign_access(&fake).await.unwrap();
    let mut fake = set.claims.clone();
    fake.aud = "wrong-audience".into();
    let wrong_audience = f.signer.sign_access(&fake).await.unwrap();
    let svc = protocol(&f, f.signer.clone());
    for token in [
        &manager.access_token,
        set.id_token.as_ref().unwrap(),
        &unknown,
        &wrong_audience,
        &SecretToken::new("not-a-token".into()).unwrap(),
    ] {
        assert!(matches!(
            svc.userinfo(token, f.signer.as_ref(), &budget(&f)).await,
            Err(IdentityError::AccessDenied)
        ));
        let response = f
            .client
            .get(format!("{}/oidc/userinfo", f.base_url))
            .bearer_auth(token.expose_secret())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
        assert_eq!(
            response.headers()["www-authenticate"],
            "Bearer error=\"invalid_token\""
        );
        assert_eq!(
            response.json::<serde_json::Value>().await.unwrap()["error"],
            "invalid_token"
        );
    }
    let response = f
        .client
        .get(format!("{}/oidc/userinfo?client_id=other", f.base_url))
        .bearer_auth(set.access_token.expose_secret())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    f.db.execute_unprepared("UPDATE oauth_consent SET scope='openid'")
        .await
        .unwrap();
    assert!(
        svc.userinfo(&set.access_token, f.signer.as_ref(), &budget(&f))
            .await
            .is_err()
    );
    f.db.execute_unprepared("UPDATE oauth_consent SET scope='openid profile'")
        .await
        .unwrap();
    f.db.execute_unprepared("UPDATE identity_access_generation SET generation=generation+1")
        .await
        .unwrap();
    assert!(
        svc.userinfo(&set.access_token, f.signer.as_ref(), &budget(&f))
            .await
            .is_err()
    );
    f.db.execute_unprepared("UPDATE identity_access_generation SET generation=generation-1")
        .await
        .unwrap();
    assert!(
        svc.userinfo(&set.access_token, f.signer.as_ref(), &budget(&f))
            .await
            .is_ok()
    );
    let existing_budget = budget(&f);
    f.clock.set_synchronized(false);
    assert!(matches!(
        svc.userinfo(&set.access_token, f.signer.as_ref(), &existing_budget)
            .await,
        Err(IdentityError::DependencyUnavailable)
    ));
    f.close().await;
}
