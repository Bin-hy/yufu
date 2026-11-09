use super::{
    super::common::IdentityFixture,
    identity_support::*,
    identity_token::{VERIFIER, code, exchange, protocol, verified},
};
use auth::security::{credential::CredentialRef, identity::SecretToken};
use cobotusr::{
    domain::identity::{account::SecretDigest, error::IdentityError},
    service::{
        identity::repository::IdentityRepository,
        security::lifecycle::{RefreshRequest, SessionTokenSet},
    },
};
use sea_orm::ConnectionTrait;
use uuid::Uuid;

pub(super) async fn issued(
    f: &IdentityFixture,
    client: &str,
    root_secret: &str,
    scopes: &str,
) -> SessionTokenSet {
    let code = code(f, client, root_secret, scopes).await;
    let secret = SecretToken::new("fixture-client-secret".into()).unwrap();
    let verifier = SecretToken::new(VERIFIER.into()).unwrap();
    protocol(f, f.signer.clone())
        .exchange(
            exchange(&code, client, Some(&secret), &verifier),
            Uuid::new_v4(),
            &budget(f),
        )
        .await
        .unwrap()
        .tokens
}
fn refresh<'a>(
    set: &'a SessionTokenSet,
    client: &'a str,
    secret: &'a SecretToken,
) -> RefreshRequest<'a> {
    RefreshRequest {
        refresh_token: &set.refresh_token,
        client_id: client,
        client_secret: Some(secret),
    }
}
#[tokio::test]
async fn identity_protocol_refresh_rotates_pair_without_extending_roots_or_auth_time() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "rotate", true).await;
    let (root, root_secret) = center_secret(&f, a.owner).await;
    let one = issued(&f, "rotate-dev", &root_secret, "openid profile").await;
    let original = verified(
        &f,
        one.id_token.as_ref().unwrap().expose_secret(),
        "rotate-dev",
    );
    let svc = protocol(&f, f.signer.clone());
    let secret = SecretToken::new("fixture-client-secret".into()).unwrap();
    f.clock.advance(10_000);
    let two = svc
        .refresh(
            refresh(&one, "rotate-dev", &secret),
            Some("profile openid"),
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(two.scope, "openid profile");
    let two = two.tokens;
    assert_ne!(
        two.access_token.expose_secret(),
        one.access_token.expose_secret()
    );
    assert_ne!(
        two.refresh_token.expose_secret(),
        one.refresh_token.expose_secret()
    );
    assert_ne!(two.claims.jti, one.claims.jti);
    assert_eq!(two.claims.iat, one.claims.iat + 10);
    assert_eq!(two.claims.grant_id, one.claims.grant_id);
    assert_eq!(two.claims.session_id, one.claims.session_id);
    assert_eq!(two.claims.family_id, one.claims.family_id);
    assert_eq!(two.claims.bounds, one.claims.bounds);
    let current = verified(
        &f,
        two.id_token.as_ref().unwrap().expose_secret(),
        "rotate-dev",
    );
    for field in ["iss", "sub", "aud", "auth_time", "sid"] {
        assert_eq!(current[field], original[field]);
    }
    assert!(current.get("nonce").is_none());
    assert_eq!(current["iat"], two.claims.iat);
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let after = tx
        .load_browser_session(&SecretDigest::of(&root_secret))
        .await
        .unwrap()
        .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(root, after);
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM security_credential WHERE family_id='{}' AND credential_kind='refresh' AND expires_at={}",one.claims.family_id.as_ref().unwrap(),root.expires_at/1000*1000)).await,2);
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM security_credential WHERE jti='{}' AND revoked_at IS NOT NULL",one.claims.jti)).await,1);
    assert_eq!(
        scalar(&f, "SELECT COUNT(*)::bigint AS value FROM security_session").await,
        1
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM security_credential WHERE credential_kind='refresh' AND consumed_at IS NOT NULL AND replacement_jti IS NOT NULL").await,1);
    f.clock.advance(43_189_000);
    assert!(
        svc.refresh(
            refresh(&two, "rotate-dev", &secret),
            None,
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_err()
    );
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_refresh_replay_revokes_only_its_family_with_audit_and_outbox() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "replay", true).await;
    let (root, root_secret) = center_secret(&f, a.owner).await;
    let one = issued(&f, "replay-dev", &root_secret, "openid").await;
    let other = issued(&f, "replay-dev", &root_secret, "openid").await;
    let svc = protocol(&f, f.signer.clone());
    let secret = SecretToken::new("fixture-client-secret".into()).unwrap();
    let two = svc
        .refresh(
            refresh(&one, "replay-dev", &secret),
            None,
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap()
        .tokens;
    f.db.execute_unprepared("CREATE FUNCTION reject_replay_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='token.refresh.replay' THEN RAISE EXCEPTION 'fixture rejected'; END IF; RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER reject_replay_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_replay_commit()").await.unwrap();
    assert!(
        svc.refresh(
            refresh(&one, "replay-dev", &secret),
            None,
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_err()
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM security_refresh_family WHERE revoked_at IS NOT NULL").await,0);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery"
        )
        .await,
        0
    );
    f.db.execute_unprepared("DROP TRIGGER reject_replay_commit ON identity_audit_event")
        .await
        .unwrap();
    assert!(matches!(
        svc.refresh(
            refresh(&one, "replay-dev", &secret),
            None,
            Uuid::new_v4(),
            &budget(&f)
        )
        .await,
        Err(IdentityError::AccessDenied)
    ));
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM security_credential WHERE family_id='{}' AND revoked_at IS NOT NULL",one.claims.family_id.as_ref().unwrap())).await,4);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='token.refresh.replay'").await,1);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery"
        )
        .await,
        1
    );
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_sso_session WHERE id='{}' AND revoked_at IS NULL",root.id)).await,1);
    assert!(
        svc.refresh(
            refresh(&two, "replay-dev", &secret),
            None,
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_err()
    );
    assert!(
        svc.refresh(
            refresh(&other, "replay-dev", &secret),
            None,
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_ok()
    );
    let check = cobotusr::service::security::lifecycle::CredentialLifecycle::new(
        std::sync::Arc::new(
            cobotusr::service::security::repository::PgSecurityRepository::new(
                f.db.clone(),
                f.clock.clone(),
            ),
        ),
        f.clock.clone(),
    );
    assert!(
        check
            .check_registered(
                &CredentialRef::new(&f.base_url, &two.claims.jti).unwrap(),
                &budget(&f)
            )
            .await
            .is_err()
    );
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_refresh_concurrency_commits_one_rotation_and_replay_revocation() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "rotate-race", true).await;
    let (_, root) = center_secret(&f, a.owner).await;
    let one = issued(&f, "rotate-race-dev", &root, "openid").await;
    let svc = protocol(&f, f.signer.clone());
    let secret = SecretToken::new("fixture-client-secret".into()).unwrap();
    let b1 = budget(&f);
    let b2 = budget(&f);
    let (one, two) = tokio::join!(
        svc.refresh(
            refresh(&one, "rotate-race-dev", &secret),
            None,
            Uuid::new_v4(),
            &b1
        ),
        svc.refresh(
            refresh(&one, "rotate-race-dev", &secret),
            None,
            Uuid::new_v4(),
            &b2
        )
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM security_credential"
        )
        .await,
        4
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM security_credential WHERE revoked_at IS NULL"
        )
        .await,
        0
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='token.refresh'").await,1);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='token.refresh.replay'").await,1);
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_refresh_failures_do_not_consume_or_return_replacements() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "rotate-fail", true).await;
    let (root, root_secret) = center_secret(&f, a.owner).await;
    let one = issued(&f, "rotate-fail-dev", &root_secret, "openid profile").await;
    let svc = protocol(&f, f.signer.clone());
    let secret = SecretToken::new("fixture-client-secret".into()).unwrap();
    let bad = SecretToken::new("wrong-client-secret".into()).unwrap();
    assert!(matches!(
        svc.refresh(
            refresh(&one, "rotate-fail-dev", &bad),
            None,
            Uuid::new_v4(),
            &budget(&f)
        )
        .await,
        Err(IdentityError::Unauthenticated)
    ));
    for scope in ["openid", "openid profile email", "openid offline_access"] {
        assert!(matches!(
            svc.refresh(
                refresh(&one, "rotate-fail-dev", &secret),
                Some(scope),
                Uuid::new_v4(),
                &budget(&f)
            )
            .await,
            Err(IdentityError::InvalidScope)
        ));
    }
    for (invalidate, restore) in [
        (
            "UPDATE oauth_consent SET scope='openid'".into(),
            "UPDATE oauth_consent SET scope='openid profile'".into(),
        ),
        (
            format!(
                "UPDATE identity_sso_session SET revoked_at=created_at WHERE id='{}'",
                root.id
            ),
            format!(
                "UPDATE identity_sso_session SET revoked_at=NULL WHERE id='{}'",
                root.id
            ),
        ),
        (
            format!(
                "UPDATE identity_access_generation SET generation=generation+1 WHERE environment_id='{}'",
                a.development
            ),
            format!(
                "UPDATE identity_access_generation SET generation=generation-1 WHERE environment_id='{}'",
                a.development
            ),
        ),
        (
            format!(
                "UPDATE identity_app_environment SET lifecycle_epoch=lifecycle_epoch+1 WHERE id='{}'",
                a.development
            ),
            format!(
                "UPDATE identity_app_environment SET lifecycle_epoch=lifecycle_epoch-1 WHERE id='{}'",
                a.development
            ),
        ),
    ] {
        f.db.execute_unprepared(&invalidate).await.unwrap();
        assert!(
            svc.refresh(
                refresh(&one, "rotate-fail-dev", &secret),
                None,
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .is_err()
        );
        f.db.execute_unprepared(&restore).await.unwrap();
    }
    f.db.execute_unprepared("CREATE FUNCTION reject_refresh_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='token.refresh' THEN RAISE EXCEPTION 'fixture rejected'; END IF; RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER reject_refresh_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_refresh_commit()").await.unwrap();
    assert!(
        svc.refresh(
            refresh(&one, "rotate-fail-dev", &secret),
            None,
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_err()
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM security_credential"
        )
        .await,
        2
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM security_credential WHERE consumed_at IS NOT NULL OR replacement_jti IS NOT NULL OR revoked_at IS NOT NULL").await,0);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='token.refresh'").await,0);
    f.db.execute_unprepared("DROP TRIGGER reject_refresh_commit ON identity_audit_event")
        .await
        .unwrap();
    assert!(
        svc.refresh(
            refresh(&one, "rotate-fail-dev", &secret),
            None,
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_ok()
    );
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_refresh_http_returns_standard_errors_and_no_cookies() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "rotate-http", false).await;
    let (_, root) = center_secret(&f, a.owner).await;
    let one = issued(&f, "rotate-http-dev", &root, "openid profile").await;
    let fields = [
        ("grant_type", "refresh_token"),
        ("client_id", "rotate-http-dev"),
        ("refresh_token", one.refresh_token.expose_secret()),
    ];
    let response = f
        .client
        .post(format!("{}/oidc/token", f.base_url))
        .basic_auth("rotate-http-dev", Some("fixture-client-secret"))
        .form(&fields)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(!response.headers().contains_key("set-cookie"));
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["scope"], "openid profile");
    let id = verified(&f, body["id_token"].as_str().unwrap(), "rotate-http-dev");
    assert!(id.get("nonce").is_none());
    let response = f
        .client
        .post(format!("{}/oidc/token", f.base_url))
        .basic_auth("rotate-http-dev", Some("fixture-client-secret"))
        .form(&fields)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["error"],
        "invalid_grant"
    );
    let two = issued(&f, "rotate-http-dev", &root, "openid profile").await;
    let response = f
        .client
        .post(format!("{}/oidc/token", f.base_url))
        .basic_auth("rotate-http-dev", Some("fixture-client-secret"))
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", two.refresh_token.expose_secret()),
            ("scope", "openid profile email"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["error"],
        "invalid_scope"
    );
    f.close().await;
}
