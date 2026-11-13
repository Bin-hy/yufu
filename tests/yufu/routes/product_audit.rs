use super::{
    super::common::IdentityFixture,
    identity_browser::{BrowserCookies, http_login},
    identity_support::*,
};
use uuid::Uuid;
use yufu::service::identity::repository::{AuditEvent, AuditResult, IdentityRepository};

#[tokio::test]
async fn product_audit_search_filters_across_pages_and_rejects_reused_cursor() {
    let f = IdentityFixture::start().await;
    let app = app(&f, "product-audit", false).await;
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    for index in 1..=60 {
        let id = Uuid::from_u128(index);
        tx.append_audit(&AuditEvent {
            id,
            request_id: id,
            actor_id: Some(app.owner),
            action: "fixture.audit",
            object_kind: "application",
            object_id: app.app.to_string(),
            result: AuditResult::Success,
            reason: None,
            change_summary: serde_json::json!({"application_id":app.app}),
            operation_id: None,
            commit_marker: Some(id),
        })
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
    let base = format!("{}/api/v1/admin/audit-events/search", f.base_url);
    assert_eq!(f.client.get(&base).send().await.unwrap().status(), 401);
    let mut cookies = BrowserCookies::default();
    http_login(&f, &mut cookies).await;
    let first: serde_json::Value = f
        .client
        .get(&base)
        .query(&[
            ("application_id", app.app.to_string()),
            ("action", "fixture.audit".into()),
            ("limit", "25".into()),
        ])
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first["items"].as_array().unwrap().len(), 25, "{first}");
    assert_eq!(first["items"][0]["id"], Uuid::from_u128(60).to_string());
    let cursor = first["next"].as_str().unwrap();
    let next: serde_json::Value = f
        .client
        .get(&base)
        .query(&[
            ("application_id", app.app.to_string()),
            ("action", "fixture.audit".into()),
            ("cursor", cursor.into()),
        ])
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(next["items"].as_array().unwrap().len(), 25, "{next}");
    assert_eq!(next["items"][0]["id"], Uuid::from_u128(35).to_string());
    let mismatch = f
        .client
        .get(&base)
        .query(&[("action", "other"), ("cursor", cursor)])
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(mismatch.status(), 409);
    let exact: serde_json::Value = f
        .client
        .get(&base)
        .query(&[("request_id", Uuid::from_u128(1).to_string())])
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(exact["items"].as_array().unwrap().len(), 1);
    assert_eq!(exact["items"][0]["id"], Uuid::from_u128(1).to_string());
    for query in [
        "result=maybe",
        "actor=",
        "from=not-a-date",
        "unknown=ignored",
    ] {
        assert_eq!(
            f.client
                .get(format!("{base}?{query}"))
                .header("cookie", cookies.header())
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
}
