use super::{super::common::IdentityFixture, identity_support::*};
use auth::security::identity::SecretToken;
use cobotusr::service::identity::repository::IdentityRepository;
use cobotusr::{
    domain::identity::{
        account::{IssuedSecret, SecretDigest},
        application::ClientKind,
        authorization::AuthorizationParameters,
        error::IdentityError,
        session::AuthorizationOutcome,
    },
    service::identity::{
        authorization::{AuthorizationFlowProof, AuthorizationResult, AuthorizationService},
        sso::{LoginFlowProof, SsoService},
    },
    service::security::lifecycle::CodeExchangeRequest,
};
use sea_orm::ConnectionTrait;
use std::sync::Arc;
use uuid::Uuid;

fn service(f: &IdentityFixture) -> AuthorizationService {
    AuthorizationService::new(Arc::new(repository(f)), f.clock.clone(), f.config.clone()).unwrap()
}
fn parameters(client: &str, scopes: &str, prompt: Option<&str>) -> AuthorizationParameters {
    let mut uri = reqwest::Url::parse("https://identity.example/oidc/authorize").unwrap();
    let mut query = uri.query_pairs_mut();
    query.extend_pairs([
        ("client_id", client),
        ("redirect_uri", "https://rp.example/callback"),
        ("response_type", "code"),
        ("scope", scopes),
        ("state", "state + &"),
        ("nonce", "request-nonce"),
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
    AuthorizationParameters::parse(uri.query().unwrap()).unwrap()
}
fn pending(result: AuthorizationResult) -> (Uuid, SecretToken) {
    let id = match result.outcome {
        AuthorizationOutcome::RequireLogin(flow)
        | AuthorizationOutcome::RequireConsent(flow, _) => flow.id(),
        _ => panic!("expected interaction"),
    };
    (
        id,
        SecretToken::new(result.flow_proof.unwrap().into_response()).unwrap(),
    )
}
fn redirect(result: AuthorizationResult) -> reqwest::Url {
    assert!(result.flow_proof.is_none());
    reqwest::Url::parse(&match result.outcome {
        AuthorizationOutcome::RedirectToClient(response) => response.into_redirect(),
        AuthorizationOutcome::ProtocolError(error) => error.into_redirect(),
        _ => panic!("expected redirect"),
    })
    .unwrap()
}
fn field(url: &reqwest::Url, name: &str) -> Option<String> {
    url.query_pairs()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
}
fn browser() -> String {
    IssuedSecret::from_entropy(rand::random()).into_response()
}

#[tokio::test]
async fn identity_authorization_consent_completion_is_atomic_bound_and_single_use() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "auth-consent", true).await;
    let (root, secret) = center_secret(&f, a.owner).await;
    let browser = browser();
    let svc = service(&f);
    let (id, proof) = pending(
        svc.authorize(
            parameters("auth-consent-dev", "openid profile", None),
            &SecretDigest::of(&browser),
            Some(&secret),
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap(),
    );
    let summary = svc
        .inspect(
            AuthorizationFlowProof { id, proof: &proof },
            &SecretDigest::of(&browser),
            Some(&secret),
            &budget(&f),
        )
        .await
        .unwrap();
    let value = serde_json::to_value(summary).unwrap();
    assert_eq!(value["interaction"], "consent");
    for key in ["state", "nonce", "redirect_uri", "code_challenge", "proof"] {
        assert!(value.get(key).is_none());
    }
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_access_generation"
        )
        .await,
        0,
        "inspection must not create a generation"
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM oauth_authorization_code"
        )
        .await,
        0
    );
    assert!(matches!(
        svc.inspect(
            AuthorizationFlowProof { id, proof: &proof },
            &SecretDigest::of("other-browser"),
            Some(&secret),
            &budget(&f)
        )
        .await,
        Err(IdentityError::Forbidden)
    ));
    let wrong = SecretToken::new(browser.clone()).unwrap();
    assert!(matches!(
        svc.inspect(
            AuthorizationFlowProof { id, proof: &wrong },
            &SecretDigest::of(&browser),
            Some(&secret),
            &budget(&f)
        )
        .await,
        Err(IdentityError::Forbidden)
    ));
    f.db.execute_unprepared("CREATE FUNCTION reject_auth_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='authorization.issue' THEN RAISE EXCEPTION 'fixture rejected'; END IF; RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER reject_auth_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_auth_commit()").await.unwrap();
    let evidence = write_evidence(&f, Some(&secret), &browser);
    assert!(
        svc.complete(
            AuthorizationFlowProof { id, proof: &proof },
            &evidence,
            true,
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_err()
    );
    for table in [
        "oauth_consent",
        "identity_oidc_rp_session",
        "oauth_authorization_code",
        "identity_access_generation",
    ] {
        assert_eq!(
            scalar(
                &f,
                &format!("SELECT COUNT(*)::bigint AS value FROM {table}")
            )
            .await,
            0,
            "rolled back {table}"
        );
    }
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_authorization_flow WHERE completed_at IS NOT NULL").await,0);
    f.db.execute_unprepared("DROP TRIGGER reject_auth_commit ON identity_audit_event")
        .await
        .unwrap();
    let first_budget = budget(&f);
    let second_budget = budget(&f);
    let (one, two) = tokio::join!(
        svc.complete(
            AuthorizationFlowProof { id, proof: &proof },
            &evidence,
            true,
            Uuid::new_v4(),
            &first_budget
        ),
        svc.complete(
            AuthorizationFlowProof { id, proof: &proof },
            &evidence,
            true,
            Uuid::new_v4(),
            &second_budget
        )
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    let url = redirect(one.or(two).unwrap());
    assert_eq!(field(&url, "state").unwrap(), "state + &");
    let code = field(&url, "code").unwrap();
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM oauth_authorization_code c JOIN identity_oidc_rp_session r ON r.id=c.oidc_rp_session_id WHERE c.code='sha256:{}' AND c.sso_session_id=r.sso_session_id AND c.environment_id=r.environment_id AND c.config_version_id=r.config_version_id AND r.sso_session_id='{}' AND r.nonce='request-nonce'",SecretDigest::of(&code).hex(),root.id)).await,1);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM oauth_consent WHERE scope='openid profile' AND environment_id IS NOT NULL AND config_version_id IS NOT NULL").await,1);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session"
        )
        .await,
        1
    );
    assert!(
        svc.complete(
            AuthorizationFlowProof { id, proof: &proof },
            &evidence,
            true,
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_err()
    );
    f.close().await;
}

/// OIDC Core makes `nonce` and PKCE optional for a confidential client. Outline
/// sends neither, so the whole authorize -> consent -> code -> token path must
/// work without them and must not invent a nonce claim.
#[tokio::test]
async fn identity_authorization_confidential_client_without_nonce_or_pkce_exchanges_code() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "auth-no-pkce", false).await;
    let (_, secret) = center_secret(&f, a.owner).await;
    let browser = browser();
    let svc = service(&f);
    let mut uri = reqwest::Url::parse("https://identity.example/oidc/authorize").unwrap();
    uri.query_pairs_mut().extend_pairs([
        ("client_id", "auth-no-pkce-dev"),
        ("redirect_uri", "https://rp.example/callback"),
        ("response_type", "code"),
        ("scope", "openid profile email"),
        ("state", "state + &"),
    ]);
    let (id, proof) = pending(
        svc.authorize(
            AuthorizationParameters::parse(uri.query().unwrap()).unwrap(),
            &SecretDigest::of(&browser),
            Some(&secret),
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap(),
    );
    let url = redirect(
        svc.complete(
            AuthorizationFlowProof { id, proof: &proof },
            &write_evidence(&f, Some(&secret), &browser),
            true,
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap(),
    );
    assert_eq!(field(&url, "state").as_deref(), Some("state + &"));
    let code = SecretToken::new(field(&url, "code").unwrap()).unwrap();
    let client_secret = SecretToken::new("fixture-client-secret".into()).unwrap();
    let issued = super::identity_token::protocol(&f, f.signer.clone())
        .exchange(
            CodeExchangeRequest {
                code: &code,
                client_id: "auth-no-pkce-dev",
                client_secret: Some(&client_secret),
                redirect_uri: Some("https://rp.example/callback"),
                code_verifier: None,
            },
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(issued.scope, "email openid profile");
    let claims = super::identity_token::verified(
        &f,
        issued.tokens.id_token.as_ref().unwrap().expose_secret(),
        "auth-no-pkce-dev",
    );
    assert_eq!(claims["sub"], a.owner.to_string());
    assert!(claims.get("nonce").is_none());
    assert_eq!(claims["preferred_username"], "identity-admin");
    f.close().await;
}

/// A browser-only client has no client secret, so PKCE stays mandatory.
#[tokio::test]
async fn identity_authorization_browser_spa_without_pkce_is_rejected() {
    let f = IdentityFixture::start().await;
    let a = app_kind(&f, "auth-spa-no-pkce", false, ClientKind::BrowserSpa).await;
    let (_, secret) = center_secret(&f, a.owner).await;
    let svc = service(&f);
    let mut uri = reqwest::Url::parse("https://identity.example/oidc/authorize").unwrap();
    uri.query_pairs_mut().extend_pairs([
        ("client_id", "auth-spa-no-pkce-dev"),
        ("redirect_uri", "https://rp.example/callback"),
        ("response_type", "code"),
        ("scope", "openid"),
        ("state", "spa-state"),
    ]);
    let url = redirect(
        svc.authorize(
            AuthorizationParameters::parse(uri.query().unwrap()).unwrap(),
            &SecretDigest::of(&browser()),
            Some(&secret),
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap(),
    );
    assert_eq!(field(&url, "error").as_deref(), Some("invalid_request"));
    assert_eq!(field(&url, "state").as_deref(), Some("spa-state"));
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_authorization_flow"
        )
        .await,
        0
    );
    f.close().await;
}

#[tokio::test]
async fn identity_authorization_reauthentication_continues_the_original_request() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "auth-login", false).await;
    let browser = browser();
    let svc = service(&f);
    let (id, proof) = pending(
        svc.authorize(
            parameters("auth-login-dev", "openid", Some("login")),
            &SecretDigest::of(&browser),
            None,
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap(),
    );
    let (_, old_secret) = center_secret(&f, a.owner).await;
    let old_evidence = write_evidence(&f, Some(&old_secret), &browser);
    assert!(matches!(
        svc.complete(
            AuthorizationFlowProof { id, proof: &proof },
            &old_evidence,
            true,
            Uuid::new_v4(),
            &budget(&f)
        )
        .await,
        Err(IdentityError::LoginRequired)
    ));
    let sso = SsoService::new(Arc::new(repository(&f)), f.clock.clone(), f.config.clone()).unwrap();
    let flow = LoginFlowProof {
        id,
        proof: SecretToken::new(proof.expose_secret().to_owned()).unwrap(),
    };
    let login = sso
        .authenticate_password(
            "identity-admin",
            "fixture-admin-password",
            &old_evidence,
            Some(&flow),
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    let root_id = login.authentication.root.id;
    let new_secret = login.cookie_secret.into_response();
    let summary = svc
        .inspect(
            AuthorizationFlowProof { id, proof: &proof },
            &SecretDigest::of(&browser),
            Some(&new_secret),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(summary).unwrap()["interaction"],
        "consent"
    );
    let evidence = write_evidence(&f, Some(&new_secret), &browser);
    let url = redirect(
        svc.complete(
            AuthorizationFlowProof { id, proof: &proof },
            &evidence,
            true,
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap(),
    );
    assert!(field(&url, "code").is_some());
    assert_eq!(field(&url, "state").unwrap(), "state + &");
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE sso_session_id='{root_id}'")).await,1);
    f.close().await;
}

#[tokio::test]
async fn identity_authorization_noninteractive_trust_and_denial_leave_no_credentials() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "auth-none", false).await;
    let browser = browser();
    let binding = SecretDigest::of(&browser);
    let svc = service(&f);
    let url = redirect(
        svc.authorize(
            parameters("auth-none-dev", "openid", Some("none")),
            &binding,
            None,
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap(),
    );
    assert_eq!(field(&url, "error").unwrap(), "login_required");
    let (_, secret) = center_secret(&f, a.owner).await;
    let url = redirect(
        svc.authorize(
            parameters("auth-none-dev", "openid", Some("none")),
            &binding,
            Some(&secret),
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap(),
    );
    assert_eq!(field(&url, "error").unwrap(), "consent_required");
    let mut payload = parameters("auth-none-dev", "openid", None).to_payload();
    payload["redirect_uri"] = serde_json::json!("https://attacker.example/cb");
    assert!(
        svc.authorize(
            AuthorizationParameters::from_payload(payload).unwrap(),
            &binding,
            Some(&secret),
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_err()
    );
    assert!(
        svc.authorize(
            parameters("unknown", "openid", None),
            &binding,
            Some(&secret),
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_err()
    );
    let mut payload = parameters("auth-none-dev", "openid", None).to_payload();
    payload.as_object_mut().unwrap().remove("state");
    let url = redirect(
        svc.authorize(
            AuthorizationParameters::from_payload(payload).unwrap(),
            &binding,
            Some(&secret),
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap(),
    );
    assert_eq!(field(&url, "error").unwrap(), "invalid_request");
    assert!(field(&url, "state").is_none());
    let url = redirect(
        svc.authorize(
            parameters("auth-none-dev", "openid api", None),
            &binding,
            Some(&secret),
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap(),
    );
    assert_eq!(field(&url, "error").unwrap(), "invalid_scope");
    for table in [
        "identity_authorization_flow",
        "oauth_authorization_code",
        "identity_oidc_rp_session",
        "oauth_consent",
        "identity_access_generation",
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
    let (id, proof) = pending(
        svc.authorize(
            parameters("auth-none-dev", "openid", None),
            &binding,
            Some(&secret),
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap(),
    );
    let evidence = write_evidence(&f, Some(&secret), &browser);
    let url = redirect(
        svc.complete(
            AuthorizationFlowProof { id, proof: &proof },
            &evidence,
            false,
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap(),
    );
    assert_eq!(field(&url, "error").unwrap(), "access_denied");
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_authorization_flow WHERE completed_at IS NOT NULL").await,1);
    assert_eq!(
        scalar(&f, "SELECT COUNT(*)::bigint AS value FROM oauth_consent").await,
        0
    );
    f.close().await;
}

#[tokio::test]
async fn identity_authorization_sso_reuses_root_and_consent_with_independent_rp_sessions() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "auth-seamless", true).await;
    let (root, secret) = center_secret(&f, a.owner).await;
    let browser = browser();
    let binding = SecretDigest::of(&browser);
    let svc = service(&f);
    let (id, proof) = pending(
        svc.authorize(
            parameters("auth-seamless-dev", "openid profile", None),
            &binding,
            Some(&secret),
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap(),
    );
    svc.complete(
        AuthorizationFlowProof { id, proof: &proof },
        &write_evidence(&f, Some(&secret), &browser),
        true,
        Uuid::new_v4(),
        &budget(&f),
    )
    .await
    .unwrap();
    let query = parameters("auth-seamless-dev", "openid", Some("none"));
    let first_budget = budget(&f);
    let second_budget = budget(&f);
    let (one, two) = tokio::join!(
        svc.authorize(
            query.clone(),
            &binding,
            Some(&secret),
            Uuid::new_v4(),
            &first_budget
        ),
        svc.authorize(
            query,
            &binding,
            Some(&secret),
            Uuid::new_v4(),
            &second_budget
        )
    );
    let one = redirect(one.unwrap());
    let two = redirect(two.unwrap());
    assert_ne!(field(&one, "code"), field(&two, "code"));
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(DISTINCT protocol_sid)::bigint AS value FROM identity_oidc_rp_session"
        )
        .await,
        3
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_authorization_flow WHERE completed_at IS NULL").await,0);
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let after = tx
        .load_browser_session(&SecretDigest::of(&secret))
        .await
        .unwrap()
        .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(root, after);
    // Production never reuses a development consent or implicit admin test access.
    assert!(
        svc.authorize(
            parameters("auth-seamless-prod", "openid", None),
            &binding,
            Some(&secret),
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_err()
    );
    f.close().await;
}
