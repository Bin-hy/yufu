//! Admin provider configuration, secret redaction, revision control and stats.
use super::super::common::IdentityFixture;
use super::identity_browser::{BrowserCookies, http_csrf, http_login};
use auth::security::budget::Clock;
use base64::Engine;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::{Value, json};

const KEY: [u8; 32] = [9; 32];

async fn fixture() -> IdentityFixture {
    unsafe {
        std::env::set_var(
            "YUFU_EXTERNAL_SECRET_KEY",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(KEY),
        );
    }
    IdentityFixture::start_configured_with(
        |config| {
            config.external_identity.enabled = true;
        },
        |_db| async {},
    )
    .await
}

#[tokio::test]
async fn admin_configures_provider_redacts_secret_and_reports_stats() {
    let f = fixture().await;
    let mut cookies = BrowserCookies::default();
    let _ = http_login(&f, &mut cookies).await;

    let response = f
        .client
        .get(format!("{}/api/v1/admin/external-providers", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let rows = response.json::<Value>().await.unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 5);
    assert!(
        rows.as_array()
            .unwrap()
            .iter()
            .all(|row| { row.get("secret_ciphertext").is_none() && row.get("secret").is_none() })
    );

    let csrf = http_csrf(&f, &mut cookies).await;
    let response = f
        .client
        .put(format!(
            "{}/api/v1/admin/external-providers/github",
            f.base_url
        ))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", csrf)
        .json(&json!({
            "expected_revision": 0,
            "enabled": true,
            "client_id": "client-id",
            "secret": "client-secret"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body = response.text().await.unwrap();
    assert!(!body.contains("client-secret"));
    let view: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(view["provider_key"], "github");
    assert_eq!(view["secret_configured"], true);
    assert_eq!(view["enabled"], true);
    let revision = view["revision"].as_u64().unwrap();

    let response = f
        .client
        .get(format!("{}/api/v1/identity/external/providers", f.base_url))
        .send()
        .await
        .unwrap();
    let catalog = response.json::<Value>().await.unwrap();
    assert!(
        catalog
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider["key"] == "github")
    );

    let csrf = http_csrf(&f, &mut cookies).await;
    let response = f
        .client
        .put(format!(
            "{}/api/v1/admin/external-providers/github",
            f.base_url
        ))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", csrf)
        .json(&json!({
            "expected_revision": 0,
            "enabled": true,
            "client_id": "client-id"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);

    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO identity_audit_event (id,request_id,action,object_kind,object_id,happened_at,result,change_summary,commit_marker) VALUES ($1,$2,'external.login','external_identity','x',$3,'success',$4,$1)",
        [
            uuid::Uuid::new_v4().into(),
            uuid::Uuid::new_v4().into(),
            (f.clock.now().unwrap().unix_millis - 1000).into(),
            json!({"provider":"github"}).into(),
        ],
    ))
    .await
    .unwrap();
    let response = f
        .client
        .get(format!(
            "{}/api/v1/admin/external-providers/stats?window=24h",
            f.base_url
        ))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let stats = response.json::<Value>().await.unwrap();
    let github = stats["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["provider_key"] == "github")
        .unwrap();
    assert_eq!(github["success"], 1);

    let csrf = http_csrf(&f, &mut cookies).await;
    let response = f
        .client
        .put(format!(
            "{}/api/v1/admin/external-providers/github",
            f.base_url
        ))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", csrf)
        .json(&json!({
            "expected_revision": revision,
            "enabled": false,
            "client_id": "client-id"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let response = f
        .client
        .get(format!("{}/api/v1/identity/external/providers", f.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        serde_json::json!([])
    );
}
