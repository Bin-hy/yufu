//! JWKS-based JWT verification for cross-service auth.
//!
//! Pulls a JSON Web Key Set from an IdP's `/.well-known/jwks.json` (or any
//! configured URL), caches the parsed [`DecodingKey`]s by `kid`, refreshes them
//! periodically in a background task, and exposes a `verify` method consumers
//! call per request.
//!
//! Unlike [`crate::token_manager::IdTokenSigner::verify_access_token`] which
//! requires holding the private key, this works with the public JWKS only —
//! the right tool for an OIDC client.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;

use crate::claims::AccessTokenClaims;
use crate::token_manager::TokenError;

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: String,
    n: String,
    e: String,
}

/// Client that fetches a JWKS, caches it, and verifies access tokens against it.
///
/// Cheap to clone (Arc inside).
#[derive(Clone)]
pub struct JwksClient {
    jwks_url: String,
    keys: Arc<RwLock<HashMap<String, DecodingKey>>>,
    expected_aud: Option<String>,
}

impl JwksClient {
    /// Strict access-profile validation. Callers must additionally check registry status.
    /// Only configured JWKS keys are used; token-provided key URLs are never fetched.
    pub fn verify_credential(
        &self,
        token: &crate::security::identity::SecretToken,
        expected: &crate::security::credential::CredentialExpectation,
        clock: &dyn crate::security::budget::Clock,
    ) -> Result<
        crate::security::credential::VerifiedCredential,
        crate::security::error::SecurityError,
    > {
        use crate::security::error::SecurityError;
        let kid = crate::security::credential::access_key_id(token)?;
        let key = self
            .keys
            .read()
            .map_err(|_| SecurityError::DependencyUnavailable)?
            .get(&kid)
            .cloned()
            .ok_or(SecurityError::InvalidCredential)?;
        crate::security::credential::verify_access_with_key(token, expected, &key, clock)
    }

    /// Create a client and fetch the JWKS once. Errors out if the initial fetch fails;
    /// callers should treat this as a hard startup failure (or wrap it in a retry loop).
    pub async fn new(
        jwks_url: impl Into<String>,
        expected_aud: Option<String>,
    ) -> anyhow::Result<Self> {
        let client = Self {
            jwks_url: jwks_url.into(),
            keys: Arc::new(RwLock::new(HashMap::new())),
            expected_aud,
        };
        client.refresh().await?;
        Ok(client)
    }

    /// Spawn a background task that refreshes the JWKS every `interval_secs` seconds.
    /// Failures are logged but don't crash the task — stale keys keep working until exp.
    pub fn spawn_refresher(self: &Arc<Self>, interval_secs: u64) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            tick.tick().await; // skip immediate fire (we just fetched in new())
            loop {
                tick.tick().await;
                if let Err(e) = this.refresh().await {
                    tracing::warn!(error = %e, jwks_url = %this.jwks_url, "JWKS refresh failed; using cached keys");
                }
            }
        });
    }

    async fn refresh(&self) -> anyhow::Result<()> {
        let resp: Jwks = reqwest::get(&self.jwks_url)
            .await?
            .error_for_status()?
            .json()
            .await?;
        let mut new_keys = HashMap::with_capacity(resp.keys.len());
        for jwk in resp.keys {
            match DecodingKey::from_rsa_components(&jwk.n, &jwk.e) {
                Ok(key) => {
                    new_keys.insert(jwk.kid, key);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "skipping malformed JWK");
                }
            }
        }
        if new_keys.is_empty() {
            anyhow::bail!("JWKS at {} returned no usable keys", self.jwks_url);
        }
        let count = new_keys.len();
        *self.keys.write().unwrap() = new_keys;
        tracing::info!(count, jwks_url = %self.jwks_url, "JWKS refreshed");
        Ok(())
    }

    /// Verify a JWT access token. Selects the public key by the token header's `kid`.
    /// Validates signature, exp, and (if configured) audience.
    pub fn verify(&self, token: &str) -> Result<AccessTokenClaims, TokenError> {
        let header = decode_header(token).map_err(|e| match *e.kind() {
            ErrorKind::InvalidToken => TokenError::InvalidToken,
            _ => TokenError::Other(e.to_string()),
        })?;
        let kid = header.kid.ok_or(TokenError::InvalidToken)?;
        let key = {
            let map = self.keys.read().unwrap();
            map.get(&kid).cloned().ok_or(TokenError::InvalidSignature)?
        };

        let mut validation = Validation::new(Algorithm::RS256);
        match &self.expected_aud {
            Some(a) => validation.set_audience(&[a]),
            None => validation.validate_aud = false,
        }

        match decode::<AccessTokenClaims>(token, &key, &validation) {
            Ok(data) => Ok(data.claims),
            Err(err) => match *err.kind() {
                ErrorKind::ExpiredSignature => Err(TokenError::Expired),
                ErrorKind::InvalidSignature => Err(TokenError::InvalidSignature),
                ErrorKind::InvalidToken => Err(TokenError::InvalidToken),
                _ => Err(TokenError::Other(err.to_string())),
            },
        }
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;
    use crate::security::{
        budget::tests::ManualClock,
        credential::{CredentialExpectation, CredentialProfile},
        identity::{PrincipalKind, SecretToken, TokenUse},
    };
    use getrandom::SysRng;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use rsa::{
        PrivateKey,
        pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding},
    };
    use serde_json::{Value, json};
    use std::sync::OnceLock;

    fn keys() -> &'static (EncodingKey, DecodingKey) {
        static KEYS: OnceLock<(EncodingKey, DecodingKey)> = OnceLock::new();
        KEYS.get_or_init(|| {
            let private = PrivateKey::new(&mut SysRng, 2048).unwrap();
            let private_pem = private.to_pkcs8_pem(LineEnding::LF).unwrap();
            let public_pem = private
                .to_public_key()
                .to_public_key_pem(LineEnding::LF)
                .unwrap();
            (
                EncodingKey::from_rsa_pem(private_pem.as_bytes()).unwrap(),
                DecodingKey::from_rsa_pem(public_pem.as_bytes()).unwrap(),
            )
        })
    }

    fn client() -> JwksClient {
        JwksClient {
            jwks_url: "https://idp.test/.well-known/jwks.json".into(),
            keys: Arc::new(RwLock::new(HashMap::from([(
                "key-1".into(),
                keys().1.clone(),
            )]))),
            expected_aud: Some("legacy-other-audience".into()),
        }
    }
    fn expectation() -> CredentialExpectation {
        CredentialExpectation::new(
            "https://idp.test",
            "resource-app",
            "app",
            vec![CredentialProfile::new(PrincipalKind::User, TokenUse::UserSession).unwrap()],
        )
        .unwrap()
    }
    fn claims() -> Value {
        json!({"iss":"https://idp.test","sub":"42","aud":"resource-app","iat":98,"exp":200,"jti":"credential-1","principal_kind":"user","token_use":"user_session","application":"app","grant_id":"grant-1","session_id":"session-1","bounds":{"grants":[]},"binding":{"kind":"none"}})
    }
    fn header() -> Header {
        let mut h = Header::new(Algorithm::RS256);
        h.typ = Some("at+jwt".into());
        h.kid = Some("key-1".into());
        h
    }
    fn token(value: Value, h: Header) -> SecretToken {
        SecretToken::new(encode(&h, &value, &keys().0).unwrap()).unwrap()
    }

    #[test]
    fn security_t07_valid_signature_is_not_current_status_authorization() {
        let verified = client()
            .verify_credential(
                &token(claims(), header()),
                &expectation(),
                &ManualClock::new(),
            )
            .unwrap();
        assert_eq!(verified.principal().subject(), "42");
        assert_eq!(verified.reference().jti(), "credential-1");
        assert_eq!(verified.signing_key_id(), "key-1");
        assert!(verified.claims().bounds.grants().is_empty());
    }

    #[test]
    fn security_t07_wrong_issuer_audience_purpose_subject_and_app_are_rejected() {
        let c = client();
        for (field, value) in [
            ("iss", json!("https://other.test")),
            ("aud", json!("other-audience")),
            ("aud", json!(["resource-app", "other-audience"])),
            ("token_use", json!("mcp_access")),
            ("principal_kind", json!("machine")),
            ("sub", json!("")),
            ("jti", json!("")),
            ("application", json!("other")),
            ("grant_id", json!("")),
            ("binding", json!({"kind":"gateway","gateway_id":"g"})),
        ] {
            let mut raw = claims();
            raw[field] = value;
            assert!(
                c.verify_credential(&token(raw, header()), &expectation(), &ManualClock::new())
                    .is_err(),
                "field {field}"
            );
        }
        for field in [
            "iss",
            "sub",
            "aud",
            "iat",
            "exp",
            "jti",
            "token_use",
            "principal_kind",
            "application",
            "grant_id",
            "session_id",
            "bounds",
            "binding",
        ] {
            let mut missing = claims();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                c.verify_credential(
                    &token(missing, header()),
                    &expectation(),
                    &ManualClock::new()
                )
                .is_err(),
                "missing {field}"
            );
        }
    }

    #[test]
    fn security_t07_registered_profiles_have_distinct_binding_requirements() {
        let c = client();
        for (kind, use_, audience, binding) in [
            (
                PrincipalKind::User,
                TokenUse::McpAccess,
                "resource-app",
                json!({"kind":"none"}),
            ),
            (
                PrincipalKind::Machine,
                TokenUse::SecurityService,
                "yufu:security",
                json!({"kind":"gateway","gateway_id":"g"}),
            ),
            (
                PrincipalKind::Machine,
                TokenUse::PermissionSync,
                "yufu:permission-sync",
                json!({"kind":"none"}),
            ),
        ] {
            let expected = CredentialExpectation::new(
                "https://idp.test",
                audience,
                "app",
                vec![CredentialProfile::new(kind, use_).unwrap()],
            )
            .unwrap();
            let mut raw = claims();
            raw["principal_kind"] = json!(kind);
            raw["token_use"] = json!(use_);
            raw["aud"] = json!(audience);
            raw["binding"] = binding;
            raw.as_object_mut().unwrap().remove("session_id");
            assert!(
                c.verify_credential(
                    &token(raw.clone(), header()),
                    &expected,
                    &ManualClock::new()
                )
                .is_ok()
            );
        }
        assert!(CredentialProfile::new(PrincipalKind::Machine, TokenUse::UserSession).is_err());
        assert!(CredentialProfile::new(PrincipalKind::Device, TokenUse::DeviceAccess).is_err());
        assert!(CredentialProfile::new(PrincipalKind::User, TokenUse::StreamConnect).is_err());
        assert!(CredentialProfile::new(PrincipalKind::User, TokenUse::GatewayOperation).is_err());
    }

    #[test]
    fn security_t07_header_key_and_algorithm_are_not_chosen_by_attacker() {
        let c = client();
        for typ in [None, Some("JWT"), Some("yufu-execution+jwt")] {
            let mut h = header();
            h.typ = typ.map(str::to_owned);
            assert!(
                c.verify_credential(&token(claims(), h), &expectation(), &ManualClock::new())
                    .is_err()
            );
        }
        let mut h = header();
        h.kid = Some("unknown".into());
        assert!(
            c.verify_credential(&token(claims(), h), &expectation(), &ManualClock::new())
                .is_err()
        );
        let mut h = header();
        h.jku = Some("https://attacker.test/jwks".into());
        assert!(
            c.verify_credential(&token(claims(), h), &expectation(), &ManualClock::new())
                .is_err()
        );
        let mut h = Header::new(Algorithm::HS256);
        h.typ = Some("at+jwt".into());
        h.kid = Some("key-1".into());
        let hmac = SecretToken::new(
            encode(&h, &claims(), &EncodingKey::from_secret(b"not-the-rsa-key")).unwrap(),
        )
        .unwrap();
        assert!(
            c.verify_credential(&hmac, &expectation(), &ManualClock::new())
                .is_err()
        );
        let signed = token(claims(), header());
        let mut tampered = signed.expose_secret().as_bytes().to_vec();
        let i = tampered.iter().rposition(|v| *v == b'.').unwrap() + 2;
        tampered[i] = if tampered[i] == b'A' { b'B' } else { b'A' };
        assert!(
            c.verify_credential(
                &SecretToken::new(String::from_utf8(tampered).unwrap()).unwrap(),
                &expectation(),
                &ManualClock::new()
            )
            .is_err()
        );
    }

    #[test]
    fn security_t07_clock_boundaries_and_unknown_fields_fail_closed() {
        let c = client();
        for (field, value) in [
            ("exp", json!(100)),
            ("exp", json!(101)),
            ("exp", json!(u64::MAX)),
            ("iat", json!(103)),
            ("iat", json!(201)),
            ("nbf", json!(101)),
            ("scope_without_bounds", json!("*")),
        ] {
            let mut raw = claims();
            raw[field] = value;
            assert!(
                c.verify_credential(&token(raw, header()), &expectation(), &ManualClock::new())
                    .is_err(),
                "field {field}"
            );
        }
        let clock = ManualClock::new();
        clock.0.lock().unwrap().synchronized = false;
        assert!(
            c.verify_credential(&token(claims(), header()), &expectation(), &clock)
                .is_err()
        );
        assert!(
            c.verify_credential(
                &SecretToken::new("a".repeat(65_537)).unwrap(),
                &expectation(),
                &ManualClock::new()
            )
            .is_err()
        );
    }

    #[derive(Clone, Copy)]
    enum StatusMode {
        Active,
        Revoked,
        DifferentSubject,
        Expired,
        Slow,
    }
    struct TestStatusChecker(StatusMode);
    #[async_trait::async_trait]
    impl crate::security::credential::CurrentCredentialChecker for TestStatusChecker {
        async fn check_current(
            &self,
            verified: &crate::security::credential::VerifiedCredential,
            _budget: &crate::security::budget::SecurityBudget,
        ) -> Result<
            crate::security::credential::CurrentCredentialStatus,
            crate::security::error::SecurityError,
        > {
            use crate::security::credential::{CurrentCredentialStatus, DecisionValidity};
            if matches!(self.0, StatusMode::Slow) {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
            let mut claims = verified.claims().clone();
            if matches!(self.0, StatusMode::DifferentSubject) {
                claims.sub = "other-user".into();
            }
            let checked = if matches!(self.0, StatusMode::Expired) {
                90_000
            } else {
                100_000
            };
            Ok(CurrentCredentialStatus {
                claims,
                active: !matches!(self.0, StatusMode::Revoked),
                validity: DecisionValidity::from_query_start(checked, 200_000, None, None).unwrap(),
            })
        }
    }
    fn strict_state(mode: StatusMode) -> Arc<crate::security::credential::StrictAccessState> {
        Arc::new(crate::security::credential::StrictAccessState::new(
            Arc::new(client()),
            expectation(),
            Arc::new(ManualClock::new()),
            Arc::new(TestStatusChecker(mode)),
        ))
    }
    fn strict_app(mode: StatusMode) -> axum::Router {
        use crate::security::credential::AuthenticatedContext;
        use axum::{Extension, Router, middleware, routing::get};
        Router::new()
            .route(
                "/private",
                get(
                    |Extension(context): Extension<AuthenticatedContext>| async move {
                        context.principal().subject().to_owned()
                    },
                ),
            )
            .layer(middleware::from_fn_with_state(
                strict_state(mode),
                crate::strict_access_middleware,
            ))
    }

    #[tokio::test]
    async fn security_t08_bearer_is_required_and_client_context_cannot_bypass() {
        use axum::body::Body;
        use http::{Request, header::AUTHORIZATION};
        use tower::ServiceExt;
        let secret = token(claims(), header());
        let mut headers = http::HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            format!("Bearer {}", secret.expose_secret())
                .parse()
                .unwrap(),
        );
        let context = strict_state(StatusMode::Active)
            .authenticate(&headers)
            .await
            .unwrap();
        let mut request = Request::builder()
            .uri(format!("/private?access_token={}", secret.expose_secret()))
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(context);
        assert_eq!(
            strict_app(StatusMode::Active)
                .oneshot(request)
                .await
                .unwrap()
                .status(),
            http::StatusCode::UNAUTHORIZED
        );
        let request = Request::builder()
            .uri("/private")
            .header(AUTHORIZATION, format!("Bearer {}", secret.expose_secret()))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            strict_app(StatusMode::Active)
                .oneshot(request)
                .await
                .unwrap()
                .status(),
            http::StatusCode::OK
        );
        headers.append(AUTHORIZATION, "Bearer duplicate".parse().unwrap());
        assert!(
            strict_state(StatusMode::Active)
                .authenticate(&headers)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn security_t08_current_registry_evidence_must_match_and_be_fresh() {
        use axum::body::Body;
        use http::{Request, header::AUTHORIZATION};
        use tower::ServiceExt;
        let secret = token(claims(), header());
        for mode in [
            StatusMode::Revoked,
            StatusMode::DifferentSubject,
            StatusMode::Expired,
        ] {
            let request = Request::builder()
                .uri("/private")
                .header(AUTHORIZATION, format!("Bearer {}", secret.expose_secret()))
                .body(Body::empty())
                .unwrap();
            assert_eq!(
                strict_app(mode).oneshot(request).await.unwrap().status(),
                http::StatusCode::UNAUTHORIZED
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn security_t08_registry_wait_uses_security_deadline() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            http::header::AUTHORIZATION,
            format!("Bearer {}", token(claims(), header()).expose_secret())
                .parse()
                .unwrap(),
        );
        assert!(matches!(
            strict_state(StatusMode::Slow).authenticate(&headers).await,
            Err(crate::security::error::SecurityError::DeadlineExceeded)
        ));
    }

    #[tokio::test]
    async fn security_t08_existing_rs256_entries_no_longer_accept_query_bearer() {
        use axum::{Router, body::Body, middleware, routing::get};
        use http::{Request, StatusCode, header::AUTHORIZATION};
        use tower::ServiceExt;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut raw = claims();
        raw["iat"] = json!(now);
        raw["exp"] = json!(now + 3600);
        raw["aud"] = json!("legacy-other-audience");
        let rsa_token = token(raw, header());
        let app = Router::new()
            .route("/legacy", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(
                Arc::new(client()),
                crate::access_middleware_rs256,
            ));
        let query = Request::builder()
            .uri(format!(
                "/legacy?access_token={}",
                rsa_token.expose_secret()
            ))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(query).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
        let request = Request::builder()
            .uri("/legacy")
            .header(
                AUTHORIZATION,
                format!("Bearer {}", rsa_token.expose_secret()),
            )
            .body(Body::empty())
            .unwrap();
        assert_eq!(app.oneshot(request).await.unwrap().status(), StatusCode::OK);

        let signer = Arc::new(crate::token_manager::IdTokenSigner::from_hmac(
            "test-legacy-secret",
            3600,
        ));
        let local_token = signer
            .sign_access_token("42", "https://idp.test", "yufu", "", 3600, vec![], vec![])
            .unwrap();
        let app = Router::new()
            .route("/legacy", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(
                signer,
                crate::access_middleware_local_rs256,
            ));
        let query = Request::builder()
            .uri(format!("/legacy?access_token={local_token}"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(query).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
        let request = Request::builder()
            .uri("/legacy")
            .header(AUTHORIZATION, format!("Bearer {local_token}"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(app.oneshot(request).await.unwrap().status(), StatusCode::OK);
    }
}
