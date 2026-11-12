use super::{
    super::common::IdentityFixture,
    identity_browser::{BrowserCookies, http_csrf},
    identity_support::*,
};
use std::sync::Arc;
use uuid::Uuid;
use yufu::domain::identity::{account::IssuedSecret, error::IdentityError};
use yufu::service::identity::logout::{LogoutRequest, LogoutService, LogoutStart};
fn request(client: &str, hint: Option<String>) -> LogoutRequest {
    LogoutRequest {
        client_id: Some(client.into()),
        id_token_hint: hint,
        post_logout_redirect_uri: Some("https://rp.example/bye".into()),
        state: Some("state with & plus +".into()),
    }
}
#[tokio::test]
async fn identity_protocol_logout_confirms_bound_root_and_preserves_other_browser_and_exact_state()
{
    let f = IdentityFixture::start().await;
    let a = app(&f, "logout-managed", true).await;
    let (root, secret) = center_secret(&f, a.owner).await;
    let (other, other_secret) = center_secret(&f, a.owner).await;
    let (tokens, _, rp) = identity_tokens(&f, &a, &root, "openid").await;
    let (_, _, other_rp) = identity_tokens(&f, &a, &other, "openid").await;
    let s = LogoutService::new(
        Arc::new(repository(&f)),
        f.clock.clone(),
        f.signer.clone(),
        f.config.clone(),
    );
    let binding = IssuedSecret::from_entropy([21; 32]).into_response();
    let e = write_evidence(&f, Some(&secret), &binding);
    let input = request(
        "logout-managed-dev",
        Some(tokens.id_token.unwrap().expose_secret().to_owned()),
    );
    let start = s
        .start(
            Some(&secret),
            yufu::domain::identity::account::SecretDigest::of(&binding),
            input,
            &budget(&f),
        )
        .await
        .unwrap();
    let LogoutStart::Confirmation { flow, proof, .. } = start else {
        panic!("confirmation")
    };
    let proof = proof.into_response();
    assert_eq!(
        s.confirm(
            &write_evidence(&f, Some(&other_secret), &binding),
            flow.id,
            &proof,
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .err(),
        Some(IdentityError::Forbidden)
    );
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE id='{rp}' AND revoked_at IS NULL")).await,1);
    let result = s
        .confirm(&e, flow.id, &proof, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    let redirect = reqwest::Url::parse(result.redirect.as_ref().unwrap()).unwrap();
    assert!(
        redirect
            .query_pairs()
            .any(|(key, value)| key == "state" && value == "state with & plus +")
    );
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE id='{rp}' AND revoked_at IS NULL")).await,0);
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE id='{other_rp}' AND revoked_at IS NULL")).await,1);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery"
        )
        .await,
        1
    );
    assert_eq!(
        s.confirm(&e, flow.id, &proof, Uuid::new_v4(), &budget(&f))
            .await
            .err(),
        Some(IdentityError::AlreadyConsumed)
    );
    f.close().await;
}
#[tokio::test]
async fn identity_protocol_logout_rejects_foreign_hint_unregistered_redirect_and_failed_commit() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "logout-rejections", true).await;
    let (root, secret) = center_secret(&f, a.owner).await;
    let (_, other_secret) = center_secret(&f, a.owner).await;
    let (tokens, _, rp) = identity_tokens(&f, &a, &root, "openid").await;
    let hint = tokens.id_token.unwrap().expose_secret().to_owned();
    let s = LogoutService::new(
        Arc::new(repository(&f)),
        f.clock.clone(),
        f.signer.clone(),
        f.config.clone(),
    );
    let binding = IssuedSecret::from_entropy([22; 32]).into_response();
    let digest = yufu::domain::identity::account::SecretDigest::of(&binding);
    assert_eq!(
        s.start(
            Some(&other_secret),
            digest.clone(),
            request("logout-rejections-dev", Some(hint.clone())),
            &budget(&f)
        )
        .await
        .err()
        .map(|e| e.code()),
        Some("forbidden")
    );
    let mut wrong = request("logout-rejections-dev", Some(hint));
    wrong.post_logout_redirect_uri = Some("https://rp.example/callback".into());
    assert_eq!(
        s.start(Some(&secret), digest.clone(), wrong, &budget(&f))
            .await
            .err()
            .map(|e| e.code()),
        Some("invalid_input")
    );
    let start = s
        .start(
            Some(&secret),
            digest,
            request("logout-rejections-dev", None),
            &budget(&f),
        )
        .await
        .unwrap();
    let LogoutStart::Confirmation { flow, proof, .. } = start else {
        panic!("confirmation")
    };
    let e = write_evidence(&f, Some(&secret), &binding);
    use sea_orm::ConnectionTrait;
    f.db.execute_unprepared("CREATE FUNCTION reject_logout_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='session.rp_logout' THEN RAISE EXCEPTION 'fixture commit failure'; END IF; RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER reject_logout_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_logout_commit()").await.unwrap();
    let proof = proof.into_response();
    assert!(
        s.confirm(&e, flow.id, &proof, Uuid::new_v4(), &budget(&f))
            .await
            .is_err()
    );
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_action_flow WHERE id='{}' AND consumed_at IS NULL",flow.id)).await,1);
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE id='{rp}' AND revoked_at IS NULL")).await,1);
    assert_eq!(
        s.confirm(
            &e,
            flow.id,
            &IssuedSecret::from_entropy([99; 32]).into_response(),
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .err(),
        Some(IdentityError::Forbidden)
    );
    f.close().await;
}
#[tokio::test]
async fn identity_protocol_logout_http_get_has_no_revocation_and_post_needs_origin_and_csrf() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "logout-http", false).await;
    let (root, secret) = center_secret(&f, a.owner).await;
    let (tokens, _, _) = identity_tokens(&f, &a, &root, "openid").await;
    let mut cookies = BrowserCookies::default();
    cookies.0.insert("yufu-dev-sso".into(), secret);
    let csrf = http_csrf(&f, &mut cookies).await;
    let initiated = f
        .client
        .post(format!("{}/oidc/end-session", f.base_url))
        .header("cookie", cookies.header())
        .form(&[("client_id", "logout-http-dev")])
        .send()
        .await
        .unwrap();
    assert_eq!(initiated.status(), 200);
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_sso_session WHERE id='{}' AND revoked_at IS NULL",root.id)).await,1);
    let response = f
        .client
        .get(format!("{}/oidc/end-session", f.base_url))
        .query(&[
            (
                "id_token_hint",
                tokens.id_token.unwrap().expose_secret().to_owned(),
            ),
            ("post_logout_redirect_uri", "https://rp.example/bye".into()),
            ("state", "wire-state".into()),
        ])
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers()["referrer-policy"],
        "same-origin",
        "the confirm form must keep a real Origin; no-referrer makes Chromium send null"
    );
    cookies.absorb(response.headers());
    let html = response.text().await.unwrap();
    let flow = html
        .split("name=\"flow_id\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let failed = f
        .client
        .post(format!("{}/oidc/end-session", f.base_url))
        .header("cookie", cookies.header())
        .form(&[("flow_id", flow), ("csrf_token", csrf.as_str())])
        .send()
        .await
        .unwrap();
    assert_eq!(failed.status(), 403);
    let failed = f
        .client
        .post(format!("{}/oidc/end-session", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .form(&[("flow_id", flow)])
        .send()
        .await
        .unwrap();
    assert_eq!(failed.status(), 403);
    let result = f
        .client
        .post(format!("{}/oidc/end-session", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .form(&[("flow_id", flow), ("csrf_token", csrf.as_str())])
        .send()
        .await
        .unwrap();
    assert_eq!(result.status(), 303);
    assert_eq!(
        result.headers()["location"],
        "https://rp.example/bye?state=wire-state"
    );
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_logout_expired_hint_is_reference_only_and_anonymous_never_revokes() {
    use yufu::domain::identity::account::SecretDigest;
    let f = IdentityFixture::start().await;
    let a = app(&f, "logout-expired", true).await;
    let (root, secret) = center_secret(&f, a.owner).await;
    let (tokens, _, rp) = identity_tokens(&f, &a, &root, "openid").await;
    let hint = tokens.id_token.unwrap().expose_secret().to_owned();
    f.clock.advance(301_000);
    let s = LogoutService::new(
        Arc::new(repository(&f)),
        f.clock.clone(),
        f.signer.clone(),
        f.config.clone(),
    );
    let binding = IssuedSecret::from_entropy([23; 32]).into_response();
    let anonymous = s
        .start(
            None,
            SecretDigest::of(&binding),
            request("logout-expired-dev", Some(hint.clone())),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(matches!(anonymous, LogoutStart::Complete(_)));
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE id='{rp}' AND revoked_at IS NULL")).await,1);
    assert_eq!(
        s.start(
            Some(&secret),
            SecretDigest::of(&binding),
            request("another-client", Some(hint.clone())),
            &budget(&f)
        )
        .await
        .err()
        .map(|e| e.code()),
        Some("invalid_input")
    );
    let started = s
        .start(
            Some(&secret),
            SecretDigest::of(&binding),
            request("logout-expired-dev", Some(hint)),
            &budget(&f),
        )
        .await
        .unwrap();
    let LogoutStart::Confirmation { flow, proof, .. } = started else {
        panic!("expired reference")
    };
    let proof = proof.into_response();
    let evidence = write_evidence(&f, Some(&secret), &binding);
    let left_budget = budget(&f);
    let right_budget = budget(&f);
    let (left, right) = tokio::join!(
        s.confirm(&evidence, flow.id, &proof, Uuid::new_v4(), &left_budget),
        s.confirm(&evidence, flow.id, &proof, Uuid::new_v4(), &right_budget)
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='session.rp_logout'").await,1);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery"
        )
        .await,
        1
    );
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_logout_confirmation_quota_and_expiry_do_not_revoke() {
    use yufu::domain::identity::account::SecretDigest;
    let f = IdentityFixture::start().await;
    let a = app(&f, "logout-quota", false).await;
    let (root, secret) = center_secret(&f, a.owner).await;
    let s = LogoutService::new(
        Arc::new(repository(&f)),
        f.clock.clone(),
        f.signer.clone(),
        f.config.clone(),
    );
    let binding = IssuedSecret::from_entropy([24; 32]).into_response();
    let mut first = None;
    for _ in 0..8 {
        let start = s
            .start(
                Some(&secret),
                SecretDigest::of(&binding),
                request("logout-quota-dev", None),
                &budget(&f),
            )
            .await
            .unwrap();
        if first.is_none() {
            first = Some(start);
        }
    }
    assert_eq!(
        s.start(
            Some(&secret),
            SecretDigest::of(&binding),
            request("logout-quota-dev", None),
            &budget(&f)
        )
        .await
        .err()
        .map(|e| e.code()),
        Some("capacity_exceeded")
    );
    let LogoutStart::Confirmation { flow, proof, .. } = first.unwrap() else {
        panic!("confirmation")
    };
    let evidence = write_evidence(&f, Some(&secret), &binding);
    f.clock.advance(600_001);
    assert_eq!(
        s.confirm(
            &evidence,
            flow.id,
            &proof.into_response(),
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .err(),
        Some(IdentityError::Expired)
    );
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_sso_session WHERE id='{}' AND revoked_at IS NULL",root.id)).await,1);
    assert!(matches!(
        s.start(
            Some(&secret),
            SecretDigest::of(&binding),
            request("logout-quota-dev", None),
            &budget(&f)
        )
        .await
        .unwrap(),
        LogoutStart::Confirmation { .. }
    ));
    f.close().await;
}
