//! Invitation history is metadata-only; reissue adds records and never revives secrets.
use super::super::common::IdentityFixture;
use super::identity_browser::{BrowserCookies, http_login};
use super::identity_support::*;
use auth::security::{budget::Clock, identity::SecretToken};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::{Value, json};
use yufu::service::identity::repository::{
    IdentityRepository, IdentityWrite, IdentityWriteResult, NewAccountWrite,
};

fn write(
    f: &IdentityFixture,
    cookies: &BrowserCookies,
    csrf: &str,
    path: &str,
) -> reqwest::RequestBuilder {
    f.client
        .post(format!("{}/api/v1{path}", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", csrf)
}
async fn issue(
    f: &IdentityFixture,
    cookies: &BrowserCookies,
    csrf: &str,
    purpose: &str,
    person_kind: &str,
    target_user_id: Option<i32>,
) -> Value {
    let response = write(f, cookies, csrf, "/admin/invitations")
        .json(&json!({
            "purpose": purpose,
            "person_kind": person_kind,
            "developer_grant": false,
            "target_user_id": target_user_id
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    response.json().await.unwrap()
}
async fn list(
    f: &IdentityFixture,
    cookies: &BrowserCookies,
    query: &str,
) -> (reqwest::StatusCode, Value) {
    let response = f
        .client
        .get(format!("{}/api/v1/admin/invitations{query}", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.json().await.unwrap_or_else(|_| json!({}));
    (status, body)
}
async fn create_account(f: &IdentityFixture, username: &str) -> i32 {
    let mut tx = repository(f).begin(&budget(f)).await.unwrap();
    let IdentityWriteResult::Account(account) = tx
        .apply(IdentityWrite::CreateAccount(NewAccountWrite {
            username: username.into(),
            password_hash: SecretToken::new("fixture-hash".into()).unwrap(),
            nickname: username.into(),
            person_kind: yufu::domain::identity::account::PersonKind::Employee,
            developer_enabled: false,
        }))
        .await
        .unwrap()
    else {
        panic!("account expected")
    };
    tx.commit().await.unwrap();
    account.user_id
}
async fn browser_for(f: &IdentityFixture, user_id: i32) -> BrowserCookies {
    let (_, secret) = center_secret(f, user_id).await;
    let mut cookies = BrowserCookies::default();
    cookies.0.insert("yufu-dev-sso".into(), secret);
    cookies
}

#[tokio::test]
async fn invitation_list_derives_status_filters_and_requires_account_read() {
    let f = IdentityFixture::start().await;
    let mut admin = BrowserCookies::default();
    let csrf = http_login(&f, &mut admin).await;
    let actor = admin_id(&f).await;
    let now = f.clock.now().unwrap().unix_millis as i64;

    let pending = issue(&f, &admin, &csrf, "onboarding", "employee", None).await;
    let accepted = issue(&f, &admin, &csrf, "onboarding", "employee", None).await;
    let revoked = issue(&f, &admin, &csrf, "onboarding", "collaborator", None).await;
    let expired = issue(&f, &admin, &csrf, "onboarding", "employee", None).await;
    let accepted_and_revoked = issue(&f, &admin, &csrf, "onboarding", "employee", None).await;

    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE identity_invitation SET consumed_at=$2,accepted_user_id=$3 WHERE id=$1",
        [
            accepted["id"]
                .as_str()
                .unwrap()
                .parse::<uuid::Uuid>()
                .unwrap()
                .into(),
            now.into(),
            actor.into(),
        ],
    ))
    .await
    .unwrap();
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE identity_invitation SET revoked_at=$2 WHERE id=$1",
        [
            revoked["id"]
                .as_str()
                .unwrap()
                .parse::<uuid::Uuid>()
                .unwrap()
                .into(),
            now.into(),
        ],
    ))
    .await
    .unwrap();
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE identity_invitation SET created_at=$2,expires_at=$3 WHERE id=$1",
        [
            expired["id"]
                .as_str()
                .unwrap()
                .parse::<uuid::Uuid>()
                .unwrap()
                .into(),
            (now - 10_000).into(),
            (now - 1).into(),
        ],
    ))
    .await
    .unwrap();
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE identity_invitation SET consumed_at=$2,accepted_user_id=$3,revoked_at=$2 WHERE id=$1",
        [
            accepted_and_revoked["id"].as_str().unwrap().parse::<uuid::Uuid>().unwrap().into(),
            now.into(),
            actor.into(),
        ],
    ))
    .await
    .unwrap();

    let (status, body) = list(&f, &admin, "?limit=50").await;
    assert_eq!(status, 200, "{body}");
    let items = body["items"].as_array().unwrap();
    let by_id = |id: &Value| {
        items
            .iter()
            .find(|item| item["id"] == id["id"])
            .unwrap_or_else(|| panic!("missing invitation {id}"))
    };
    assert_eq!(by_id(&pending)["status"], "pending");
    assert_eq!(by_id(&accepted)["status"], "accepted");
    assert_eq!(by_id(&accepted)["accepted_user_id"], actor);
    assert_eq!(by_id(&revoked)["status"], "revoked");
    assert_eq!(by_id(&expired)["status"], "expired");
    assert_eq!(by_id(&accepted_and_revoked)["status"], "accepted");
    assert_eq!(body["next"], Value::Null);

    let (_, accepted_only) = list(&f, &admin, "?status=accepted&limit=50").await;
    assert!(
        accepted_only["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["status"] == "accepted")
    );
    let (_, collaborators) = list(&f, &admin, "?person_kind=collaborator&limit=50").await;
    assert!(
        collaborators["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["person_kind"] == "collaborator")
    );
    let (_, exact) = list(
        &f,
        &admin,
        &format!("?q={}&limit=50", pending["id"].as_str().unwrap()),
    )
    .await;
    assert_eq!(exact["items"].as_array().unwrap().len(), 1);
    let (_, qualifications) = list(&f, &admin, "?purpose=qualification&limit=50").await;
    assert!(qualifications["items"].as_array().unwrap().is_empty());

    let ordinary_id = create_account(&f, "ordinary-invitation-reader").await;
    let ordinary = browser_for(&f, ordinary_id).await;
    let (status, _) = list(&f, &ordinary, "?limit=25").await;
    assert_eq!(status, 403);
    assert!(
        body.to_string().find("secret").is_none(),
        "list must not expose secret material"
    );
    f.close().await;
}

#[tokio::test]
async fn invitation_list_paginates_without_duplicates_and_never_returns_secrets() {
    let f = IdentityFixture::start().await;
    let mut admin = BrowserCookies::default();
    let _ = http_login(&f, &mut admin).await;
    let actor = admin_id(&f).await;
    let now = f.clock.now().unwrap().unix_millis as i64;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO identity_invitation (id,secret_digest,purpose,person_kind,developer_grant,created_by,created_at,expires_at) SELECT gen_random_uuid(),decode(repeat(md5(i::text),2),'hex'),'onboarding','employee',FALSE,$1,$2+i,$2+86400000+i FROM generate_series(1,30) i",
        [actor.into(), now.into()],
    ))
    .await
    .unwrap();

    let (status, first) = list(&f, &admin, "?limit=25").await;
    assert_eq!(status, 200);
    assert_eq!(first["items"].as_array().unwrap().len(), 25);
    let next = first["next"].as_str().expect("second page expected");
    let (status, second) = list(&f, &admin, &format!("?limit=25&after={next}")).await;
    assert_eq!(status, 200);
    let second_items = second["items"].as_array().unwrap();
    assert!(!second_items.is_empty());
    let mut ids = std::collections::HashSet::new();
    for item in first["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(second_items)
    {
        assert!(ids.insert(item["id"].as_str().unwrap().to_owned()));
        let encoded = item.to_string();
        assert!(!encoded.contains("secret"), "{encoded}");
        assert!(!encoded.contains("digest"), "{encoded}");
    }
    assert_eq!(ids.len(), 30);
    f.close().await;
}

#[tokio::test]
async fn invitation_reissue_keeps_source_and_allows_parallel_links() {
    let f = IdentityFixture::start().await;
    let mut admin = BrowserCookies::default();
    let csrf = http_login(&f, &mut admin).await;
    let source = issue(&f, &admin, &csrf, "onboarding", "employee", None).await;
    let source_id = source["id"].as_str().unwrap().to_owned();

    let first = write(
        &f,
        &admin,
        &csrf,
        &format!("/admin/invitations/{source_id}/reissue"),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(first.status(), 200);
    let first: Value = first.json().await.unwrap();
    assert_ne!(first["id"], source["id"]);
    assert_ne!(first["secret"], source["secret"]);
    let second = write(
        &f,
        &admin,
        &csrf,
        &format!("/admin/invitations/{source_id}/reissue"),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(second.status(), 200);
    let second: Value = second.json().await.unwrap();
    assert_ne!(second["id"], first["id"]);
    assert_ne!(second["secret"], first["secret"]);

    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_invitation WHERE consumed_at IS NULL AND revoked_at IS NULL"
        )
        .await,
        3
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='account.invitation.reissue' AND result='success'"
        )
        .await,
        2
    );
    for secret in [&source["secret"], &first["secret"], &second["secret"]] {
        assert_eq!(
            write(&f, &admin, &csrf, "/identity/actions/exchange")
                .json(&json!({"kind":"invitation","secret":secret}))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    }
    let source_row = f
        .db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT consumed_at,revoked_at,purpose,person_kind FROM identity_invitation WHERE id=$1",
            [source_id.parse::<uuid::Uuid>().unwrap().into()],
        ))
        .await
        .unwrap()
        .unwrap();
    use sea_orm::TryGetable;
    assert!(
        Option::<i64>::try_get(&source_row, "", "consumed_at")
            .unwrap()
            .is_none()
    );
    assert!(
        Option::<i64>::try_get(&source_row, "", "revoked_at")
            .unwrap()
            .is_none()
    );

    let target = create_account(&f, "reissue-target").await;
    let qualified = issue(&f, &admin, &csrf, "qualification", "employee", Some(target)).await;
    let before = scalar(
        &f,
        "SELECT COUNT(*)::bigint AS value FROM identity_invitation",
    )
    .await;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET enabled=FALSE WHERE id=$1",
        [target.into()],
    ))
    .await
    .unwrap();
    let rejected = write(
        &f,
        &admin,
        &csrf,
        &format!(
            "/admin/invitations/{}/reissue",
            qualified["id"].as_str().unwrap()
        ),
    )
    .send()
    .await
    .unwrap();
    assert!(rejected.status().is_client_error());
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_invitation"
        )
        .await,
        before
    );
    f.close().await;
}

#[tokio::test]
async fn invitation_reissue_and_revoke_require_exact_permissions() {
    let f = IdentityFixture::start().await;
    let mut admin = BrowserCookies::default();
    let csrf = http_login(&f, &mut admin).await;
    let actor = admin_id(&f).await;
    let source = issue(&f, &admin, &csrf, "onboarding", "employee", None).await;
    let revocable = issue(&f, &admin, &csrf, "onboarding", "employee", None).await;
    let before = scalar(
        &f,
        "SELECT COUNT(*)::bigint AS value FROM identity_invitation",
    )
    .await;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) SELECT $1,id,FALSE,NOW(),NOW() FROM authority WHERE method='POST' AND authority_uri='/admin/invitations'",
        [actor.into()],
    ))
    .await
    .unwrap();
    let reissue = write(
        &f,
        &admin,
        &csrf,
        &format!(
            "/admin/invitations/{}/reissue",
            source["id"].as_str().unwrap()
        ),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(reissue.status(), 403);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_invitation"
        )
        .await,
        before
    );
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) SELECT $1,id,FALSE,NOW(),NOW() FROM authority WHERE method='POST' AND authority_uri='/admin/invitations/{id}/revoke'",
        [actor.into()],
    ))
    .await
    .unwrap();
    let revoke = write(
        &f,
        &admin,
        &csrf,
        &format!(
            "/admin/invitations/{}/revoke",
            revocable["id"].as_str().unwrap()
        ),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(revoke.status(), 403);
    assert_eq!(
        scalar(
            &f,
            &format!(
                "SELECT COUNT(*)::bigint AS value FROM identity_invitation WHERE id='{}' AND revoked_at IS NULL",
                revocable["id"].as_str().unwrap()
            )
        )
        .await,
        1
    );
    f.close().await;
}
