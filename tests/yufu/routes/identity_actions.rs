//! Black-box tests of account actions; opaque proofs travel only in HttpOnly cookies.
use super::super::common::IdentityFixture;
use super::identity_browser::{BrowserCookies, http_csrf, http_login};
use super::identity_support::*;
use axum::http::header::SET_COOKIE;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::{Value, json};

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
async fn invitation(f: &IdentityFixture, cookies: &BrowserCookies, csrf: &str) -> Value {
    let response = write(f, cookies, csrf, "/admin/invitations")
        .json(&json!({"purpose":"onboarding","person_kind":"collaborator","developer_grant":false}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(!response.headers().contains_key(SET_COOKIE));
    response.json().await.unwrap()
}

#[tokio::test]
async fn identity_actions_http_admin_authorization_is_method_specific_and_deny_wins() {
    let f = IdentityFixture::start().await;
    let mut cookies = BrowserCookies::default();
    let csrf = http_login(&f, &mut cookies).await;
    let issued = invitation(&f, &cookies, &csrf).await;
    assert_eq!(issued["secret"].as_str().unwrap().len(), 43);
    let actor = admin_id(&f).await;
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    use yufu::service::identity::repository::{
        IdentityRepository, IdentityWrite, IdentityWriteResult, NewAccountWrite,
    };
    let IdentityWriteResult::Account(member) = tx
        .apply(IdentityWrite::CreateAccount(NewAccountWrite {
            username: "ordinary-actions".into(),
            password_hash: auth::security::identity::SecretToken::new("fixture-hash".into())
                .unwrap(),
            nickname: "Ordinary".into(),
            person_kind: yufu::domain::identity::account::PersonKind::Employee,
            developer_enabled: false,
        }))
        .await
        .unwrap()
    else {
        panic!("account expected")
    };
    tx.commit().await.unwrap();
    let (_, secret) = center_secret(&f, member.user_id).await;
    let mut ordinary = BrowserCookies::default();
    ordinary.0.insert("yufu-dev-sso".into(), secret);
    let ordinary_csrf = http_csrf(&f, &mut ordinary).await;
    let response = write(&f, &ordinary, &ordinary_csrf, "/admin/password-resets")
        .json(&json!({"user_id":actor}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) SELECT $1,id,true,NOW(),NOW() FROM authority WHERE method='POST' AND authority_uri='/admin/invitations'",[member.user_id.into()])).await.unwrap();
    let granted = invitation(&f, &ordinary, &ordinary_csrf).await;
    assert!(granted["secret"].is_string());
    f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) SELECT $1,id,false,NOW(),NOW() FROM authority WHERE method='POST' AND authority_uri='/admin/invitations'", [actor.into()])).await.unwrap();
    let response = write(&f, &cookies, &csrf, "/admin/invitations")
        .json(&json!({"purpose":"onboarding","person_kind":"employee","developer_grant":false}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    assert!(!response.headers().contains_key(SET_COOKIE));
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_invitation"
        )
        .await,
        2
    );
    // A denial of invitation issuance does not deny independently registered reset issuance.
    let response = write(&f, &cookies, &csrf, "/admin/password-resets")
        .json(&json!({"user_id":actor}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    f.db.execute_unprepared(
        "DELETE FROM authority_role WHERE authority_id IN (SELECT id FROM authority WHERE authority_uri='/admin/password-resets'); DELETE FROM authority WHERE authority_uri='/admin/password-resets'")
        .await.unwrap();
    let response = write(&f, &cookies, &csrf, "/admin/password-resets")
        .json(&json!({"user_id":actor}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        403,
        "even superadmin needs a registered operation"
    );
    let response = f
        .client
        .delete(format!("{}/api/v1/admin/invitations", f.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 405);
    f.close().await;
}

#[tokio::test]
async fn identity_actions_http_exchange_inspect_and_accept_bind_proof_to_browser() {
    let f = IdentityFixture::start().await;
    let mut admin = BrowserCookies::default();
    let admin_csrf = http_login(&f, &mut admin).await;
    let issued = invitation(&f, &admin, &admin_csrf).await;
    let mut guest = BrowserCookies::default();
    let csrf = http_csrf(&f, &mut guest).await;
    let response = write(&f, &guest, &csrf, "/identity/actions/exchange")
        .json(&json!({"kind":"invitation","secret":issued["secret"],"redirect_uri":"https://untrusted.example"}))
        .send().await.unwrap();
    assert_eq!(response.status(), 400);
    let response = write(&f, &guest, &csrf, "/identity/actions/exchange")
        .json(&json!({"kind":"invitation","secret":issued["secret"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let cookie = response
        .headers()
        .get(SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("Path=/") && !cookie.contains("Domain=")
    );
    guest.absorb(response.headers());
    let exchanged: Value = response.json().await.unwrap();
    let encoded = exchanged.to_string();
    assert!(!encoded.contains(issued["secret"].as_str().unwrap()));
    assert!(exchanged.get("proof").is_none() && exchanged.get("secret").is_none());
    let id = exchanged["action_id"].as_str().unwrap();
    let path = format!("{}/api/v1/identity/actions/{id}", f.base_url);
    let (first, second) = tokio::join!(
        f.client.get(&path).header("cookie", guest.header()).send(),
        f.client.get(&path).header("cookie", guest.header()).send()
    );
    for response in [first.unwrap(), second.unwrap()] {
        assert_eq!(response.status(), 200);
        assert!(!response.headers().contains_key(SET_COOKIE));
        let view: Value = response.json().await.unwrap();
        assert_eq!(view["csrf_token"], csrf);
        assert_eq!(view["person_kind"], "collaborator");
        assert!(view.get("proof").is_none());
    }
    let mut other = BrowserCookies::default();
    let _ = http_csrf(&f, &mut other).await;
    let proof_name = format!("yufu-dev-action-{id}");
    other
        .0
        .insert(proof_name.clone(), guest.0[&proof_name].clone());
    let response = f
        .client
        .get(&path)
        .header("cookie", other.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    let response = write(&f, &guest, &csrf, "/identity/invitations/accept")
        .json(&json!({"action_id":id,"existing_account":false,"username":"invited-http","password":" a password with spaces ","nickname":"Invited","role_id":1}))
        .send().await.unwrap();
    assert_eq!(response.status(), 400);
    let response = write(&f, &guest, &csrf, "/identity/invitations/accept")
        .json(&json!({"action_id":id,"existing_account":false,"username":"invited-http","password":" a password with spaces ","nickname":"Invited"}))
        .send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()[SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    let result: Value = response.json().await.unwrap();
    assert_eq!(result["requires_login"], true);
    assert!(result.get("access_token").is_none());
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_app_assignment"
        )
        .await,
        0
    );
    assert_eq!(scalar(&f, "SELECT COUNT(*)::bigint AS value FROM identity_invitation WHERE consumed_at IS NOT NULL").await, 1);
    let response = f
        .client
        .get(&path)
        .header("cookie", guest.header())
        .send()
        .await
        .unwrap();
    assert!(response.status().is_client_error());
    f.close().await;
}

#[tokio::test]
async fn identity_actions_http_password_commit_failure_keeps_cookies_and_reset_clears_own_root() {
    let f = IdentityFixture::start().await;
    let mut cookies = BrowserCookies::default();
    let csrf = http_login(&f, &mut cookies).await;
    assert_eq!(
        http_csrf(&f, &mut cookies).await,
        csrf,
        "center CSRF is reused after login"
    );
    let response = f
        .client
        .patch(format!("{}/api/v1/account/profile", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .json(&json!({"nickname":"Changed","email":"real@example.test","email_verified":true}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    let response = f
        .client
        .patch(format!("{}/api/v1/account/profile", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .json(&json!({"nickname":"Changed","email":"real@example.test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<Value>().await.unwrap()["email_verified"],
        false
    );
    f.db.execute_unprepared(
        "CREATE FUNCTION fail_http_password_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='account.password.change' THEN RAISE EXCEPTION 'fixture commit rejected'; END IF; RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER fail_http_password_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_http_password_commit()")
        .await.unwrap();
    let response = write(&f, &cookies, &csrf, "/account/password")
        .json(&json!({"old_password":"fixture-admin-password","new_password":"changed-http-password"})).send().await.unwrap();
    assert!(response.status().is_server_error());
    assert!(!response.headers().contains_key(SET_COOKIE));
    assert_eq!(
        scalar(&f, "SELECT SUM(auth_epoch)::bigint AS value FROM \"user\"").await,
        0
    );
    let response = f
        .client
        .get(format!("{}/api/v1/account/session", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    f.db.execute_unprepared(
        "DROP TRIGGER fail_http_password_commit ON identity_audit_event; DROP FUNCTION fail_http_password_commit()")
        .await.unwrap();
    let actor = admin_id(&f).await;
    let response = write(&f, &cookies, &csrf, "/admin/password-resets")
        .json(&json!({"user_id":actor}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let issued: Value = response.json().await.unwrap();
    let response = write(&f, &cookies, &csrf, "/identity/actions/exchange")
        .json(&json!({"kind":"password_reset","secret":issued["secret"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    cookies.absorb(response.headers());
    let exchanged: Value = response.json().await.unwrap();
    let response = write(&f, &cookies, &csrf, "/identity/password-resets/complete")
        .json(&json!({"action_id":exchanged["action_id"],"new_password":"reset-http-password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let deleted: Vec<_> = response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap())
        .collect();
    assert_eq!(deleted.len(), 3);
    assert!(deleted.iter().all(|v| v.contains("Max-Age=0")));
    cookies.absorb(response.headers());
    let response = f
        .client
        .get(format!("{}/api/v1/account/session", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    assert_eq!(
        scalar(&f, "SELECT SUM(auth_epoch)::bigint AS value FROM \"user\"").await,
        1
    );
    // A separate browser proves that the old password was not retained by a
    // successful reset, then exercises the real self-service change endpoint.
    let mut fresh = BrowserCookies::default();
    let fresh_csrf = http_csrf(&f, &mut fresh).await;
    let response = write(&f, &fresh, &fresh_csrf, "/identity/login")
        .json(&json!({"username":"identity-admin","password":"reset-http-password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    fresh.absorb(response.headers());
    let csrf = response.json::<Value>().await.unwrap()["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let response = write(&f, &fresh, &csrf, "/account/password")
        .json(&json!({"old_password":"reset-http-password","new_password":" personal spaces "}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers().get_all(SET_COOKIE).iter().count(), 2);
    assert_eq!(
        scalar(&f, "SELECT SUM(auth_epoch)::bigint AS value FROM \"user\"").await,
        2
    );
    f.close().await;
}

#[tokio::test]
async fn identity_actions_http_account_governance_requires_revision_and_exact_authority() {
    use yufu::service::identity::repository::{
        IdentityRepository, IdentityWrite, IdentityWriteResult, NewAccountWrite,
    };
    let f = IdentityFixture::start().await;
    let mut cookies = BrowserCookies::default();
    let csrf = http_login(&f, &mut cookies).await;
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let IdentityWriteResult::Account(target) = tx
        .apply(IdentityWrite::CreateAccount(NewAccountWrite {
            username: "governance-http".into(),
            password_hash: auth::security::identity::SecretToken::new("fixture-hash".into())
                .unwrap(),
            nickname: "Target".into(),
            person_kind: yufu::domain::identity::account::PersonKind::Employee,
            developer_enabled: false,
        }))
        .await
        .unwrap()
    else {
        panic!("account expected")
    };
    tx.commit().await.unwrap();
    let patch = |path: &str| {
        f.client
            .patch(format!("{}/api/v1{path}", f.base_url))
            .header("cookie", cookies.header())
            .header("origin", &f.base_url)
            .header("x-csrf-token", &csrf)
    };
    let developer = format!("/admin/accounts/{}/developer", target.user_id);
    let enabled = format!("/admin/accounts/{}/enabled", target.user_id);
    let response = patch(&developer)
        .json(&json!({"developer_enabled":true,"expected_revision":0,"role_id":1}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    let response = patch(&developer)
        .json(&json!({"developer_enabled":true,"expected_revision":0}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.json::<Value>().await.unwrap()["revision"], 1);
    let response = patch(&enabled)
        .json(&json!({"enabled":false,"expected_revision":0}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    let response = patch(&enabled)
        .json(&json!({"enabled":false,"expected_revision":1}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        !response.headers().contains_key(SET_COOKIE),
        "disabling another person does not clear the caller's cookies"
    );
    let result = response.json::<Value>().await.unwrap();
    assert_eq!(result["auth_epoch"], 1);
    assert_eq!(result["requires_login"], false);
    let actor = admin_id(&f).await;
    f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) SELECT $1,id,false,NOW(),NOW() FROM authority WHERE method='PATCH' AND authority_uri='/admin/accounts/{id}/enabled'",[actor.into()])).await.unwrap();
    let response = patch(&enabled)
        .json(&json!({"enabled":true,"expected_revision":2}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    assert_eq!(
        scalar(
            &f,
            &format!(
                "SELECT COUNT(*)::bigint AS value FROM \"user\" WHERE id={} AND NOT enabled",
                target.user_id
            )
        )
        .await,
        1
    );
    f.close().await;
}
