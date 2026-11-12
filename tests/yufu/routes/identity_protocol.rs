//! OIDC authorization, token, introspection and protocol response scenarios.
use axum::response::IntoResponse;
use yufu::{domain::identity::error::IdentityError, error::AppError};

#[tokio::test]
async fn identity_protocol_id_tokens_have_client_specific_audience_sid_and_scope_limited_profile() {
    use super::{super::common::IdentityFixture, identity_support::*};
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
    use sha2::{Digest, Sha256};
    let f = IdentityFixture::start().await;
    let a = app(&f, "signed-a", false).await;
    let b = app(&f, "signed-b", false).await;
    let root = center(&f, a.owner).await;
    let (one, context_a, _) = identity_tokens(&f, &a, &root, "openid").await;
    let (two, context_b, _) = identity_tokens(&f, &b, &root, "openid profile email").await;
    let jwks: JwkSet = serde_json::from_str(&f.signer.jwks_json().unwrap()).unwrap();
    let mut actuals = Vec::new();
    for (set, context) in [(&one, &context_a), (&two, &context_b)] {
        let token = set.id_token.as_ref().unwrap().expose_secret();
        let header = decode_header(token).unwrap();
        assert_eq!(header.alg, Algorithm::RS256);
        let key =
            DecodingKey::from_jwk(jwks.find(header.kid.as_deref().unwrap()).unwrap()).unwrap();
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&f.base_url]);
        validation.set_audience(&[&context.client_id]);
        let actual = decode::<serde_json::Value>(token, &key, &validation)
            .unwrap()
            .claims;
        assert_eq!(actual["sub"], root.user_id.to_string());
        assert_eq!(actual["auth_time"], root.authenticated_at / 1_000);
        assert_eq!(actual["sid"], context.protocol_sid.to_string());
        assert_eq!(actual["nonce"], context.nonce.as_ref().unwrap().as_str());
        assert_eq!(
            actual["at_hash"],
            URL_SAFE_NO_PAD
                .encode(&Sha256::digest(set.access_token.expose_secret().as_bytes())[..16])
        );
        for forbidden in ["email", "email_verified", "roles", "permissions", "bounds"] {
            assert!(
                actual.get(forbidden).is_none(),
                "unexpected claim: {forbidden}"
            );
        }
        assert!(set.claims.bounds.grants().is_empty());
        actuals.push(actual);
    }
    assert_ne!(actuals[0]["aud"], actuals[1]["aud"]);
    assert_ne!(actuals[0]["sid"], actuals[1]["sid"]);
    assert!(actuals[0].get("name").is_none());
    assert!(actuals[0].get("preferred_username").is_none());
    assert_eq!(actuals[1]["preferred_username"], "identity-admin");
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_error_is_fixed_correlated_and_not_cacheable() {
    let request_id = uuid::Uuid::new_v4();
    let response = AppError::identity(IdentityError::CommitUncertain, request_id).into_response();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-request-id"], request_id.to_string());
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"code":"commit_uncertain","request_id":request_id})
    );
    let response =
        AppError::from(anyhow::anyhow!("database password=should-never-escape")).into_response();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), b"internal_server_error");
}
