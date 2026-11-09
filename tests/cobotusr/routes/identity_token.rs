use super::{super::common::IdentityFixture, identity_support::*};
use auth::security::{
    credential::SecurityAccessClaims, error::SecurityError, identity::SecretToken,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use cobotusr::{
    domain::identity::{
        account::{IssuedSecret, SecretDigest},
        application::ClientKind,
        authorization::AuthorizationParameters,
        error::IdentityError,
        session::{AuthorizationOutcome, BrowserSessionSnapshot},
    },
    service::{
        identity::{
            authorization::{AuthorizationFlowProof, AuthorizationService},
            protocol::ProtocolService,
        },
        security::{
            lifecycle::{CodeExchangeRequest, CredentialLifecycle, SessionTokenSigner},
            repository::PgSecurityRepository,
        },
    },
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

pub(super) const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
pub(super) fn protocol(
    f: &IdentityFixture,
    signer: Arc<dyn SessionTokenSigner>,
) -> ProtocolService {
    ProtocolService::new(
        Arc::new(repository(f)),
        CredentialLifecycle::new(
            Arc::new(PgSecurityRepository::new(f.db.clone(), f.clock.clone())),
            f.clock.clone(),
        ),
        f.clock.clone(),
        signer,
        f.config.clone(),
    )
    .unwrap()
}
pub(super) fn verified(f: &IdentityFixture, token: &str, audience: &str) -> serde_json::Value {
    let jwks: JwkSet = serde_json::from_str(&f.signer.jwks_json().unwrap()).unwrap();
    let header = decode_header(token).unwrap();
    assert_eq!(header.alg, Algorithm::RS256);
    let key = DecodingKey::from_jwk(jwks.find(header.kid.as_deref().unwrap()).unwrap()).unwrap();
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[&f.base_url]);
    validation.set_audience(&[audience]);
    decode::<serde_json::Value>(token, &key, &validation)
        .unwrap()
        .claims
}
pub(super) async fn code(
    f: &IdentityFixture,
    client: &str,
    secret: &str,
    scopes: &str,
) -> SecretToken {
    let browser = IssuedSecret::from_entropy(rand::random()).into_response();
    let mut url = reqwest::Url::parse("https://identity.example/oidc/authorize").unwrap();
    url.query_pairs_mut().extend_pairs([
        ("client_id", client),
        ("redirect_uri", "https://rp.example/callback"),
        ("response_type", "code"),
        ("scope", scopes),
        ("state", "fixture-state"),
        ("nonce", "fixture-nonce"),
        ("code_challenge_method", "S256"),
        (
            "code_challenge",
            &URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes())),
        ),
    ]);
    let svc = AuthorizationService::new(Arc::new(repository(f)), f.clock.clone(), f.config.clone())
        .unwrap();
    let result = svc
        .authorize(
            AuthorizationParameters::parse(url.query().unwrap()).unwrap(),
            &SecretDigest::of(&browser),
            Some(secret),
            Uuid::new_v4(),
            &budget(f),
        )
        .await
        .unwrap();
    let outcome = match result.outcome {
        AuthorizationOutcome::RequireConsent(flow, _) => {
            let proof = SecretToken::new(result.flow_proof.unwrap().into_response()).unwrap();
            svc.complete(
                AuthorizationFlowProof {
                    id: flow.id(),
                    proof: &proof,
                },
                &write_evidence(f, Some(secret), &browser),
                true,
                Uuid::new_v4(),
                &budget(f),
            )
            .await
            .unwrap()
            .outcome
        }
        outcome => outcome,
    };
    let AuthorizationOutcome::RedirectToClient(response) = outcome else {
        panic!("expected actual authorization code")
    };
    let url = reqwest::Url::parse(&response.into_redirect()).unwrap();
    SecretToken::new(
        url.query_pairs()
            .find(|(k, _)| k == "code")
            .unwrap()
            .1
            .into_owned(),
    )
    .unwrap()
}
pub(super) fn exchange<'a>(
    code: &'a SecretToken,
    client: &'a str,
    secret: Option<&'a SecretToken>,
    verifier: &'a SecretToken,
) -> CodeExchangeRequest<'a> {
    CodeExchangeRequest {
        code,
        client_id: client,
        client_secret: secret,
        redirect_uri: Some("https://rp.example/callback"),
        code_verifier: Some(verifier),
    }
}
async fn root_remaining(f: &IdentityFixture, root: &mut BrowserSessionSnapshot, millis: u64) {
    use auth::security::budget::Clock;
    root.expires_at = f.clock.now().unwrap().unix_millis + millis;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE identity_sso_session SET expires_at=$2 WHERE id=$1",
        [root.id.into(), (root.expires_at as i64).into()],
    ))
    .await
    .unwrap();
}

#[tokio::test]
async fn identity_protocol_exchange_binds_registry_rsa_claims_and_root_deadline() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "exchange", true).await;
    let (mut root, root_secret) = center_secret(&f, a.owner).await;
    root_remaining(&f, &mut root, 25_000).await;
    let code = code(&f, "exchange-dev", &root_secret, "openid profile").await;
    let verifier = SecretToken::new(VERIFIER.into()).unwrap();
    let secret = SecretToken::new("fixture-client-secret".into()).unwrap();
    let svc = protocol(&f, f.signer.clone());
    let issued = svc
        .exchange(
            exchange(&code, "exchange-dev", Some(&secret), &verifier),
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(issued.scope, "openid profile");
    let set = issued.tokens;
    assert_eq!(set.claims.exp, root.expires_at / 1000);
    assert!(set.claims.bounds.grants().is_empty());
    let access = verified(&f, set.access_token.expose_secret(), "urn:test:Development");
    assert_eq!(access["jti"], set.claims.jti);
    let id = verified(
        &f,
        set.id_token.as_ref().unwrap().expose_secret(),
        "exchange-dev",
    );
    assert_eq!(id["sub"], a.owner.to_string());
    assert_eq!(id["auth_time"], root.authenticated_at / 1000);
    assert_eq!(id["nonce"], "fixture-nonce");
    assert_eq!(id["preferred_username"], "identity-admin");
    assert!(id.get("email").is_none());
    assert_eq!(
        id["at_hash"],
        URL_SAFE_NO_PAD.encode(&Sha256::digest(set.access_token.expose_secret().as_bytes())[..16])
    );
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM security_credential c JOIN security_session s ON s.id=c.session_id JOIN identity_oidc_rp_session r ON r.id=s.oidc_rp_session_id WHERE c.issuer='{}' AND c.oidc_client_id='exchange-dev' AND s.sso_session_id='{}' AND c.revocation_parent_jti IS NULL AND c.expires_at<={} AND r.protocol_sid='{}'",f.base_url,root.id,root.expires_at,id["sid"].as_str().unwrap())).await,2);
    assert!(matches!(
        svc.exchange(
            exchange(&code, "exchange-dev", Some(&secret), &verifier),
            Uuid::new_v4(),
            &budget(&f)
        )
        .await,
        Err(IdentityError::AccessDenied)
    ));
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='token.exchange'").await,1);
    f.close().await;
}

struct FailId(Arc<auth::token_manager::IdTokenSigner>);
#[async_trait::async_trait]
impl SessionTokenSigner for FailId {
    fn key_id(&self) -> &str {
        &self.0.kid
    }
    async fn sign_access(&self, c: &SecurityAccessClaims) -> Result<SecretToken, SecurityError> {
        SessionTokenSigner::sign_access(self.0.as_ref(), c).await
    }
    async fn sign_id(
        &self,
        _: &SecurityAccessClaims,
        _: &str,
        _: Option<&str>,
        _: &SecretToken,
    ) -> Result<SecretToken, SecurityError> {
        Err(SecurityError::DependencyUnavailable)
    }
}

#[tokio::test]
async fn identity_protocol_exchange_failures_rollback_code_pair_and_audit() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "exchange-atomic", false).await;
    let (_, root) = center_secret(&f, a.owner).await;
    let code = code(&f, "exchange-atomic-dev", &root, "openid").await;
    let verifier = SecretToken::new(VERIFIER.into()).unwrap();
    let secret = SecretToken::new("fixture-client-secret".into()).unwrap();
    let svc = protocol(&f, f.signer.clone());
    let bad_secret = SecretToken::new("wrong-secret".into()).unwrap();
    assert!(matches!(
        svc.exchange(
            exchange(&code, "exchange-atomic-dev", Some(&bad_secret), &verifier),
            Uuid::new_v4(),
            &budget(&f)
        )
        .await,
        Err(IdentityError::Unauthenticated)
    ));
    let bad_verifier = SecretToken::new("x".repeat(43)).unwrap();
    assert!(
        svc.exchange(
            exchange(&code, "exchange-atomic-dev", Some(&secret), &bad_verifier),
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_err()
    );
    let mut wrong_redirect = exchange(&code, "exchange-atomic-dev", Some(&secret), &verifier);
    wrong_redirect.redirect_uri = Some("https://rp.example/other");
    assert!(
        svc.exchange(wrong_redirect, Uuid::new_v4(), &budget(&f))
            .await
            .is_err()
    );
    assert!(
        protocol(&f, Arc::new(FailId(f.signer.clone())))
            .exchange(
                exchange(&code, "exchange-atomic-dev", Some(&secret), &verifier),
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .is_err()
    );
    f.db.execute_unprepared("CREATE FUNCTION reject_exchange_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='token.exchange' THEN RAISE EXCEPTION 'fixture rejected'; END IF; RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER reject_exchange_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_exchange_commit()").await.unwrap();
    assert!(
        svc.exchange(
            exchange(&code, "exchange-atomic-dev", Some(&secret), &verifier),
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_err()
    );
    for table in [
        "security_credential",
        "security_grant",
        "security_refresh_family",
        "security_session",
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
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM oauth_authorization_code WHERE used=TRUE"
        )
        .await,
        0
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='token.exchange'").await,0);
    f.db.execute_unprepared("DROP TRIGGER reject_exchange_commit ON identity_audit_event")
        .await
        .unwrap();
    let first = budget(&f);
    let second = budget(&f);
    let (one, two) = tokio::join!(
        svc.exchange(
            exchange(&code, "exchange-atomic-dev", Some(&secret), &verifier),
            Uuid::new_v4(),
            &first
        ),
        svc.exchange(
            exchange(&code, "exchange-atomic-dev", Some(&secret), &verifier),
            Uuid::new_v4(),
            &second
        )
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM security_credential"
        )
        .await,
        2
    );
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_exchange_rechecks_consent_rp_epoch_version_and_expiry() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "exchange-current", false).await;
    let (root, root_secret) = center_secret(&f, a.owner).await;
    let code = code(&f, "exchange-current-dev", &root_secret, "openid profile").await;
    let verifier = SecretToken::new(VERIFIER.into()).unwrap();
    let secret = SecretToken::new("fixture-client-secret".into()).unwrap();
    let svc = protocol(&f, f.signer.clone());
    for (invalidate, restore) in [
        (
            "UPDATE oauth_consent SET scope='openid'".into(),
            "UPDATE oauth_consent SET scope='openid profile'".into(),
        ),
        (
            "UPDATE identity_oidc_rp_session SET revoked_at=created_at".into(),
            "UPDATE identity_oidc_rp_session SET revoked_at=NULL".into(),
        ),
        (
            format!(
                "UPDATE \"user\" SET auth_epoch=auth_epoch+1 WHERE id={}",
                a.owner
            ),
            format!(
                "UPDATE \"user\" SET auth_epoch=auth_epoch-1 WHERE id={}",
                a.owner
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
                "UPDATE identity_sso_session SET revoked_at=created_at WHERE id='{}'",
                root.id
            ),
            format!(
                "UPDATE identity_sso_session SET revoked_at=NULL WHERE id='{}'",
                root.id
            ),
        ),
    ] {
        f.db.execute_unprepared(&invalidate).await.unwrap();
        assert!(
            svc.exchange(
                exchange(&code, "exchange-current-dev", Some(&secret), &verifier),
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .is_err()
        );
        assert_eq!(
            scalar(
                &f,
                "SELECT COUNT(*)::bigint AS value FROM oauth_authorization_code WHERE used=TRUE"
            )
            .await,
            0
        );
        assert_eq!(
            scalar(
                &f,
                "SELECT COUNT(*)::bigint AS value FROM security_credential"
            )
            .await,
            0
        );
        f.db.execute_unprepared(&restore).await.unwrap();
    }
    f.clock.advance(61_000);
    assert!(
        svc.exchange(
            exchange(&code, "exchange-current-dev", Some(&secret), &verifier),
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_err()
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM oauth_authorization_code WHERE used=TRUE"
        )
        .await,
        0
    );
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_token_http_supports_basic_post_and_spa_without_browser_cookies() {
    let f = IdentityFixture::start().await;
    let web = app(&f, "token-web", false).await;
    app_kind(&f, "token-spa", false, ClientKind::BrowserSpa).await;
    let (_, root) = center_secret(&f, web.owner).await;
    for (client, method) in [
        ("token-web-dev", "basic"),
        ("token-web-dev", "post"),
        ("token-spa-dev", "none"),
    ] {
        let code = code(&f, client, &root, "openid profile").await;
        let mut fields = vec![
            ("grant_type", "authorization_code"),
            ("client_id", client),
            ("code", code.expose_secret()),
            ("redirect_uri", "https://rp.example/callback"),
            ("code_verifier", VERIFIER),
        ];
        if method == "post" {
            fields.push(("client_secret", "fixture-client-secret"));
        }
        let request = f
            .client
            .post(format!("{}/oidc/token", f.base_url))
            .form(&fields);
        let response = if method == "basic" {
            request.basic_auth(client, Some("fixture-client-secret"))
        } else {
            request
        }
        .send()
        .await
        .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["referrer-policy"], "no-referrer");
        assert!(!response.headers().contains_key("set-cookie"));
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["scope"], "openid profile");
        assert_eq!(body["token_type"], "Bearer");
        verified(&f, body["id_token"].as_str().unwrap(), client);
        let response = f
            .client
            .post(format!("{}/oidc/token", f.base_url))
            .form(&fields);
        let response = if method == "basic" {
            response.basic_auth(client, Some("fixture-client-secret"))
        } else {
            response
        }
        .send()
        .await
        .unwrap();
        assert_eq!(response.status(), 400);
        assert_eq!(
            response.json::<serde_json::Value>().await.unwrap()["error"],
            "invalid_grant"
        );
    }
    let code = code(&f, "token-web-dev", &root, "openid").await;
    let fields = [
        ("grant_type", "authorization_code"),
        ("client_id", "token-web-dev"),
        ("client_secret", "fixture-client-secret"),
        ("code", code.expose_secret()),
        ("redirect_uri", "https://rp.example/callback"),
        ("code_verifier", VERIFIER),
    ];
    let response = f
        .client
        .post(format!("{}/oidc/token", f.base_url))
        .basic_auth("token-web-dev", Some("fixture-client-secret"))
        .form(&fields)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    assert!(!response.headers().contains_key("www-authenticate"));
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["error"],
        "invalid_request"
    );
    let response = f
        .client
        .post(format!("{}/oidc/token", f.base_url))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("x".repeat(16_385))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM oauth_authorization_code WHERE used=FALSE"
        )
        .await,
        1
    );
    f.close().await;
}
