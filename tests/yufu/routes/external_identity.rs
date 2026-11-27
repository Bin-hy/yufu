//! External federation is additive: disabled by default and invisible to OIDC clients.
use super::super::common::IdentityFixture;
use super::identity_support::scalar;

#[tokio::test]
async fn external_identity_schema_starts_disabled_and_keeps_oidc_discovery() {
    let f = IdentityFixture::start().await;

    let response = f
        .client
        .get(format!("{}/api/v1/identity/external/providers", f.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!([])
    );

    for table in [
        "identity_external_identity",
        "identity_external_flow",
        "identity_session_client",
        "identity_handoff_ticket",
    ] {
        assert_eq!(
            scalar(
                &f,
                &format!("SELECT COUNT(*)::bigint AS value FROM {table}")
            )
            .await,
            0,
            "{table} should start empty"
        );
    }
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_external_provider"
        )
        .await,
        5
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_external_provider WHERE enabled"
        )
        .await,
        0
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM authority WHERE authority_uri LIKE '/admin/external-providers%'"
        )
        .await,
        2
    );

    let discovery = f
        .client
        .get(format!("{}/.well-known/openid-configuration", f.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(discovery.status(), 200);
    let body = discovery.json::<serde_json::Value>().await.unwrap();
    assert_eq!(body["issuer"], f.base_url);
    assert_eq!(body["response_types_supported"][0], "code");
}
