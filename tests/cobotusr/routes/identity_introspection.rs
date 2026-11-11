use super::{
    super::common::IdentityFixture,
    identity_refresh::issued,
    identity_support::*,
    identity_token::{VERIFIER, code, exchange, protocol},
};
use auth::security::identity::SecretToken;
use cobotusr::service::identity::repository::IdentityRepository;
use cobotusr::service::security::lifecycle::RefreshRequest;
use cobotusr::{
    domain::identity::{application::ClientKind, error::IdentityError},
    service::identity::{
        protocol::ProtocolClientAuthentication,
        reader::{ReaderLocation, ReaderService},
    },
};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use std::sync::Arc;
use uuid::Uuid;

fn client<'a>(id: &'a str, secret: &'a SecretToken) -> ProtocolClientAuthentication<'a> {
    ProtocolClientAuthentication {
        id,
        secret: Some(secret),
    }
}
fn secret() -> SecretToken {
    SecretToken::new("fixture-client-secret".into()).unwrap()
}

#[tokio::test]
async fn identity_protocol_introspection_uses_registry_for_access_and_refresh_and_hides_other_clients()
 {
    let f = IdentityFixture::start().await;
    let a = app(&f, "inspect-a", false).await;
    app(&f, "inspect-b", false).await;
    let (_, root) = center_secret(&f, a.owner).await;
    let one = issued(&f, "inspect-a-dev", &root, "openid email").await;
    let svc = protocol(&f, f.signer.clone());
    let secret = secret();
    for token in [&one.access_token, &one.refresh_token] {
        let response = svc
            .introspect(
                client("inspect-a-dev", &secret),
                token,
                f.signer.as_ref(),
                &budget(&f),
            )
            .await
            .unwrap();
        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value["active"], true);
        assert_eq!(value["scope"], "email openid");
        assert_eq!(value["client_id"], "inspect-a-dev");
        assert_eq!(value["sub"], a.owner.to_string());
        assert_eq!(value["aud"], one.claims.aud);
        assert_eq!(value["iss"], f.base_url);
        assert!(value["sid"].as_str().is_some());
        assert!(value.get("email").is_none());
        assert_eq!(
            scalar(&f, "SELECT COUNT(*)::bigint AS value FROM oauth_token").await,
            0
        );
        let other = svc
            .introspect(
                client("inspect-b-dev", &secret),
                token,
                f.signer.as_ref(),
                &budget(&f),
            )
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(other).unwrap(),
            serde_json::json!({"active":false})
        );
    }
    for token in [
        one.id_token.as_ref().unwrap(),
        &SecretToken::new("unknown".into()).unwrap(),
    ] {
        let response = svc
            .introspect(
                client("inspect-a-dev", &secret),
                token,
                f.signer.as_ref(),
                &budget(&f),
            )
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            serde_json::json!({"active":false})
        );
    }
    let response = f
        .client
        .post(format!("{}/oidc/introspect", f.base_url))
        .basic_auth("inspect-a-dev", Some("fixture-client-secret"))
        .form(&[
            ("token", one.access_token.expose_secret()),
            ("token_type_hint", "refresh_token"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(!response.headers().contains_key("set-cookie"));
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["active"],
        true
    );
    f.db.execute_unprepared("UPDATE oauth_consent SET scope='openid'")
        .await
        .unwrap();
    for token in [&one.access_token, &one.refresh_token] {
        assert!(
            !svc.introspect(
                client("inspect-a-dev", &secret),
                token,
                f.signer.as_ref(),
                &budget(&f)
            )
            .await
            .unwrap()
            .active
        );
    }
    f.db.execute_unprepared("UPDATE oauth_consent SET scope='openid email'")
        .await
        .unwrap();
    f.db.execute_unprepared("UPDATE identity_access_generation SET generation=generation+1")
        .await
        .unwrap();
    assert!(
        !svc.introspect(
            client("inspect-a-dev", &secret),
            &one.access_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await
        .unwrap()
        .active
    );
    let original_budget = budget(&f);
    f.clock.set_synchronized(false);
    assert!(matches!(
        svc.introspect(
            client("inspect-a-dev", &secret),
            &one.access_token,
            f.signer.as_ref(),
            &original_budget
        )
        .await,
        Err(IdentityError::DependencyUnavailable)
    ));
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_reader_is_bound_rotatable_and_not_a_token_or_revocation_client() {
    let f = IdentityFixture::start().await;
    let a = app_kind(&f, "reader-spa", false, ClientKind::BrowserSpa).await;
    let web = app(&f, "reader-web", false).await;
    let (root, root_secret) = center_secret(&f, a.owner).await;
    let reader_service = ReaderService::new(Arc::new(repository(&f)), f.clock.clone());
    let browser = cobotusr::domain::identity::account::IssuedSecret::from_entropy(rand::random())
        .into_response();
    let evidence = write_evidence(&f, Some(&root_secret), &browser);
    let reader = reader_service
        .create(
            &evidence,
            ReaderLocation {
                application_id: a.app,
                environment_id: a.development,
            },
            "urn:test:Development",
            "reader-service-0",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    let reader_id = reader.reader_id;
    let reader_secret = SecretToken::new(reader.secret.unwrap().into_response()).unwrap();
    let code = code(&f, "reader-spa-dev", &root_secret, "openid profile").await;
    let verifier = SecretToken::new(VERIFIER.into()).unwrap();
    let svc = protocol(&f, f.signer.clone());
    let web_tokens = issued(&f, "reader-web-dev", &root_secret, "openid").await;
    assert!(
        !svc.introspect(
            client(&reader_id, &reader_secret),
            &web_tokens.access_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await
        .unwrap()
        .active
    );
    let web_reader = reader_service
        .create(
            &evidence,
            ReaderLocation {
                application_id: web.app,
                environment_id: web.development,
            },
            "urn:test:Development",
            "reader-service-1",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    let web_reader_id = web_reader.reader_id;
    let web_reader_secret = SecretToken::new(web_reader.secret.unwrap().into_response()).unwrap();
    assert!(
        svc.introspect(
            client(&web_reader_id, &web_reader_secret),
            &web_tokens.access_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await
        .unwrap()
        .active
    );
    assert!(
        reader_service
            .change(
                &evidence,
                &reader_id,
                ReaderLocation {
                    application_id: a.app,
                    environment_id: a.production
                },
                reader.revision,
                true,
                false,
                "reader-service-2",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .is_err()
    );
    let tokens = svc
        .exchange(
            exchange(&code, "reader-spa-dev", None, &verifier),
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap()
        .tokens;
    assert!(
        svc.introspect(
            client(&reader_id, &reader_secret),
            &tokens.access_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await
        .unwrap()
        .active
    );
    assert!(matches!(
        svc.introspect(
            ProtocolClientAuthentication {
                id: "reader-spa-dev",
                secret: None
            },
            &tokens.access_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await,
        Err(IdentityError::Unauthenticated)
    ));
    for path in ["/oidc/token", "/oidc/revoke"] {
        let fields = if path.ends_with("token") {
            vec![
                ("grant_type", "refresh_token"),
                ("refresh_token", tokens.refresh_token.expose_secret()),
            ]
        } else {
            vec![("token", tokens.access_token.expose_secret())]
        };
        let response = f
            .client
            .post(format!("{}{path}", f.base_url))
            .basic_auth(&reader_id, Some(reader_secret.expose_secret()))
            .form(&fields)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
    }
    assert!(
        reader_service
            .create(
                &evidence,
                ReaderLocation {
                    application_id: a.app,
                    environment_id: a.development
                },
                "wrong",
                "reader-service-3",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .is_err()
    );
    let changed = reader_service
        .change(
            &evidence,
            &reader_id,
            ReaderLocation {
                application_id: a.app,
                environment_id: a.development,
            },
            reader.revision,
            true,
            true,
            "reader-service-4",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    let new_secret = SecretToken::new(changed.secret.unwrap().into_response()).unwrap();
    assert!(matches!(
        svc.introspect(
            client(&reader_id, &reader_secret),
            &tokens.access_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await,
        Err(IdentityError::Unauthenticated)
    ));
    assert!(
        svc.introspect(
            client(&reader_id, &new_secret),
            &tokens.access_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await
        .unwrap()
        .active
    );
    f.db.execute_unprepared(
        "UPDATE security_session_client SET enabled=false WHERE client_id='reader-spa-dev'",
    )
    .await
    .unwrap();
    assert_eq!(
        svc.introspect(
            client(&reader_id, &new_secret),
            &tokens.access_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await
        .err(),
        Some(IdentityError::Unauthenticated)
    );
    f.db.execute_unprepared(
        "UPDATE security_session_client SET enabled=true WHERE client_id='reader-spa-dev'",
    )
    .await
    .unwrap();
    reader_service
        .change(
            &evidence,
            &reader_id,
            ReaderLocation {
                application_id: a.app,
                environment_id: a.development,
            },
            changed.revision,
            false,
            false,
            "reader-service-5",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(matches!(
        svc.introspect(
            client(&reader_id, &new_secret),
            &tokens.access_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await,
        Err(IdentityError::Unauthenticated)
    ));
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    assert_eq!(
        tx.load_browser_session(&root.secret_digest)
            .await
            .unwrap()
            .unwrap(),
        root
    );
    tx.rollback().await.unwrap();
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action LIKE 'reader.%'").await,4);
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_revoke_commits_grant_audit_and_outbox_without_revoking_center_or_other_grants()
 {
    let f = IdentityFixture::start().await;
    let a = app(&f, "revoke-a", true).await;
    app(&f, "revoke-b", true).await;
    let (root, root_secret) = center_secret(&f, a.owner).await;
    let one = issued(&f, "revoke-a-dev", &root_secret, "openid").await;
    let sibling = issued(&f, "revoke-a-dev", &root_secret, "openid").await;
    let other = issued(&f, "revoke-b-dev", &root_secret, "openid").await;
    let svc = protocol(&f, f.signer.clone());
    let secret = secret();
    let cross = f
        .client
        .post(format!("{}/oidc/revoke", f.base_url))
        .basic_auth("revoke-b-dev", Some(secret.expose_secret()))
        .form(&[("token", one.access_token.expose_secret())])
        .send()
        .await
        .unwrap();
    assert_eq!(cross.status(), 200);
    assert!(
        svc.introspect(
            client("revoke-a-dev", &secret),
            &one.access_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await
        .unwrap()
        .active
    );
    let response = f
        .client
        .post(format!("{}/oidc/revoke", f.base_url))
        .basic_auth("revoke-a-dev", Some(secret.expose_secret()))
        .form(&[
            ("token", one.refresh_token.expose_secret()),
            ("token_type_hint", "access_token"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    for token in [&one.access_token, &one.refresh_token] {
        assert!(
            !svc.introspect(
                client("revoke-a-dev", &secret),
                token,
                f.signer.as_ref(),
                &budget(&f)
            )
            .await
            .unwrap()
            .active
        );
    }
    assert!(
        svc.introspect(
            client("revoke-a-dev", &secret),
            &sibling.access_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await
        .unwrap()
        .active
    );
    assert!(
        svc.introspect(
            client("revoke-b-dev", &secret),
            &other.access_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await
        .unwrap()
        .active
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='token.revoke'"
        )
        .await,
        1
    );
    let audit =
        f.db.query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT actor_id,change_summary FROM identity_audit_event WHERE action='token.revoke'"
                .to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(audit.try_get::<Option<i32>>("", "actor_id").unwrap(), None);
    let summary: serde_json::Value = audit.try_get("", "change_summary").unwrap();
    assert_eq!(summary["client_id"], "revoke-a-dev");
    assert_eq!(summary["target_user_id"], a.owner);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery"
        )
        .await,
        1
    );
    for token in [one.refresh_token.expose_secret(), "unknown"] {
        let response = f
            .client
            .post(format!("{}/oidc/revoke", f.base_url))
            .basic_auth("revoke-a-dev", Some(secret.expose_secret()))
            .form(&[("token", token)])
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='token.revoke'"
        )
        .await,
        1
    );
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    assert_eq!(
        tx.load_browser_session(&root.secret_digest)
            .await
            .unwrap()
            .unwrap(),
        root
    );
    tx.rollback().await.unwrap();
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_revoke_failed_commit_keeps_tokens_active_and_no_success_audit() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "revoke-fail", true).await;
    let (_, root) = center_secret(&f, a.owner).await;
    let one = issued(&f, "revoke-fail-dev", &root, "openid").await;
    f.db.execute_unprepared("CREATE FUNCTION reject_revoke_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='token.revoke' THEN RAISE EXCEPTION 'fixture rejected commit'; END IF; RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER reject_revoke_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_revoke_commit()").await.unwrap();
    let response = f
        .client
        .post(format!("{}/oidc/revoke", f.base_url))
        .basic_auth("revoke-fail-dev", Some("fixture-client-secret"))
        .form(&[("token", one.access_token.expose_secret())])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    assert!(
        protocol(&f, f.signer.clone())
            .introspect(
                client("revoke-fail-dev", &secret()),
                &one.access_token,
                f.signer.as_ref(),
                &budget(&f)
            )
            .await
            .unwrap()
            .active
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='token.revoke'"
        )
        .await,
        0
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery"
        )
        .await,
        0
    );
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_introspection_expiry_consumption_authentication_and_database_failures() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "inspect-errors", false).await;
    let (_, root) = center_secret(&f, a.owner).await;
    let one = issued(&f, "inspect-errors-dev", &root, "openid profile").await;
    let svc = protocol(&f, f.signer.clone());
    let secret = secret();
    for fields in [
        vec![("token", "unknown"), ("client_id", "inspect-errors-dev")],
        vec![
            ("token", "unknown"),
            ("client_id", "inspect-errors-dev"),
            ("client_secret", "wrong"),
        ],
        vec![
            ("token", "unknown"),
            ("client_id", "unknown"),
            ("client_secret", "wrong"),
        ],
    ] {
        let response = f
            .client
            .post(format!("{}/oidc/introspect", f.base_url))
            .form(&fields)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
        let value = response.json::<serde_json::Value>().await.unwrap();
        assert_eq!(value["error"], "invalid_client");
        assert!(value.get("sub").is_none());
    }
    for body in [
        "client_id=inspect-errors-dev&client_secret=fixture-client-secret&token=unknown&%74oken=other",
        "client_id=inspect-errors-dev&client_secret=fixture-client-secret&token=unknown&request_uri=forbidden",
    ] {
        let response = f
            .client
            .post(format!("{}/oidc/introspect", f.base_url))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
    }
    f.clock.advance(300_000);
    assert!(
        !svc.introspect(
            client("inspect-errors-dev", &secret),
            &one.access_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await
        .unwrap()
        .active
    );
    assert!(
        svc.introspect(
            client("inspect-errors-dev", &secret),
            &one.refresh_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await
        .unwrap()
        .active
    );
    let two = svc
        .refresh(
            RefreshRequest {
                client_id: "inspect-errors-dev",
                client_secret: Some(&secret),
                refresh_token: &one.refresh_token,
            },
            None,
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap()
        .tokens;
    assert!(
        !svc.introspect(
            client("inspect-errors-dev", &secret),
            &one.refresh_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await
        .unwrap()
        .active
    );
    f.db.execute_unprepared(
        "ALTER TABLE security_credential RENAME TO fixture_unavailable_credential",
    )
    .await
    .unwrap();
    let response = f
        .client
        .post(format!("{}/oidc/introspect", f.base_url))
        .basic_auth("inspect-errors-dev", Some("fixture-client-secret"))
        .form(&[("token", two.access_token.expose_secret())])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(
        response
            .json::<serde_json::Value>()
            .await
            .unwrap()
            .get("active")
            .is_none()
    );
    f.db.execute_unprepared(
        "ALTER TABLE fixture_unavailable_credential RENAME TO security_credential",
    )
    .await
    .unwrap();
    assert!(
        svc.introspect(
            client("inspect-errors-dev", &secret),
            &two.access_token,
            f.signer.as_ref(),
            &budget(&f)
        )
        .await
        .unwrap()
        .active
    );
    f.close().await;
}
