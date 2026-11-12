use super::{
    super::common::IdentityFixture,
    identity_browser::{BrowserCookies, http_login},
    identity_support::*,
};
use cobotusr::{
    domain::identity::{
        account::{IssuedSecret, ManagementMode},
        application::{AppEnvironmentKind, ClientKind, ConsentPolicy, LogoutCapability},
        error::IdentityError,
        registration::{CreateAdminApplication, CreateApplication},
    },
    service::identity::{
        application::{ApplicationMutation, ApplicationService},
        repository::*,
    },
};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use std::sync::Arc;
use uuid::Uuid;
fn request(owner: i32, key: &str) -> CreateAdminApplication {
    let mut production =
        config(AppEnvironmentKind::Production, ClientKind::ServerWeb, false).to_input();
    production.consent_policy = ConsentPolicy::EnterprisePreconsent;
    CreateAdminApplication {
        application: CreateApplication {
            app_key: key.into(),
            name: "Partner service".into(),
            description: None,
            third_party: true,
            development: config(
                AppEnvironmentKind::Development,
                ClientKind::ServerWeb,
                false,
            )
            .to_input(),
            production,
        },
        owner_user_id: owner,
        assignments: vec![AssignmentTarget::User(owner)],
        accept_logout_limitation: false,
    }
}
#[tokio::test]
async fn identity_application_admin_registration_requires_explicit_permission_and_keeps_owner_without_developer_qualification()
 {
    let f = IdentityFixture::start().await;
    let owner = admin_id(&f).await;
    let (_, secret) = center_secret(&f, owner).await;
    let e = write_evidence(
        &f,
        Some(&secret),
        &IssuedSecret::from_entropy([17; 32]).into_response(),
    );
    let s = ApplicationService::new(Arc::new(repository(&f)), f.clock.clone());
    let mut input = request(owner, "admin-third-party");
    input.application.production.logout_capability = LogoutCapability::LocalOnly;
    assert_eq!(
        s.register_administrator(&e, input.clone(), "no-accept", Uuid::new_v4(), &budget(&f))
            .await
            .err(),
        Some(IdentityError::InvalidInput)
    );
    input.accept_logout_limitation = true;
    let first = s
        .register_administrator(
            &e,
            input.clone(),
            "admin-register",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(first.management_mode, ManagementMode::AdminConfigured);
    assert_eq!(first.newly_issued_secrets.len(), 2);
    assert!(
        first
            .environments
            .iter()
            .all(|env| env.active_version_id.is_some())
    );
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM \"user\" WHERE id={owner} AND developer_enabled")).await,0);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_admin_activation WHERE logout_limitation_accepted").await,1);
    let repeat = s
        .register_administrator(&e, input, "admin-register", Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    assert_eq!(first.operation_id, repeat.operation_id);
    assert!(repeat.newly_issued_secrets.is_empty());
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM application WHERE management_state='managed'"
        )
        .await,
        1
    );
    f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) SELECT $1,id,FALSE,NOW(),NOW() FROM authority WHERE method='POST' AND authority_uri='/admin/apps'",[owner.into()])).await.unwrap();
    assert_eq!(
        s.register_administrator(
            &e,
            request(owner, "denied-partner"),
            "denied",
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
async fn identity_application_metadata_keeps_config_session_and_pending_review_unchanged() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "metadata-app", true).await;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE application SET management_mode='developer_managed' WHERE id=$1",
        [a.app.into()],
    ))
    .await
    .unwrap();
    let root = center(&f, a.owner).await;
    let (_, _, rp) = identity_tokens(&f, &a, &root, "openid").await;
    let (_, secret) = center_secret(&f, a.owner).await;
    let e = write_evidence(
        &f,
        Some(&secret),
        &IssuedSecret::from_entropy([18; 32]).into_response(),
    );
    let s = ApplicationService::new(Arc::new(repository(&f)), f.clock.clone());
    let submit = s
        .mutate(
            &e,
            a.app,
            ApplicationMutation::SubmitProduction {
                environment_id: a.production,
                version_id: a.prod_version,
                expected_revision: 0,
            },
            "metadata-submit",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    let approval: Uuid =
        serde_json::from_value(submit.receipt.summary["approval_id"].clone()).unwrap();
    let changed = s
        .mutate(
            &e,
            a.app,
            ApplicationMutation::UpdateMetadata {
                name: "New label".into(),
                description: Some("New description".into()),
                expected_revision: 0,
            },
            "metadata-change",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(changed.receipt.summary["affected_count"], 0);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_app_config_version"
        )
        .await,
        2
    );
    assert_eq!(
        scalar(
            &f,
            &format!(
                "SELECT lifecycle_epoch AS value FROM identity_app_environment WHERE id='{}'",
                a.development
            )
        )
        .await,
        0
    );
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE id='{rp}' AND revoked_at IS NULL")).await,1);
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_app_approval WHERE id='{approval}' AND status='pending'")).await,1);
    f.close().await;
}
#[tokio::test]
async fn identity_application_admin_and_assignment_http_require_center_csrf_and_bind_target_paths()
{
    let f = IdentityFixture::start().await;
    let mut cookies = BrowserCookies::default();
    let csrf = http_login(&f, &mut cookies).await;
    let owner = admin_id(&f).await;
    let first = f
        .client
        .post(format!("{}/api/v1/admin/apps", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .header("idempotency-key", "http-admin-register")
        .json(&request(owner, "http-partner"))
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 200);
    assert!(!first.headers().contains_key("set-cookie"));
    let first: serde_json::Value = first.json().await.unwrap();
    let group = f
        .client
        .post(format!("{}/api/v1/admin/groups", f.base_url))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .header("idempotency-key", "http-group")
        .json(&serde_json::json!({"name":"HTTP group","description":null}))
        .send()
        .await
        .unwrap();
    assert_eq!(group.status(), 200);
    let group: serde_json::Value = group.json().await.unwrap();
    let id = group["summary"]["group_id"].as_str().unwrap();
    let member = f
        .client
        .patch(format!(
            "{}/api/v1/admin/groups/{id}/members/{owner}",
            f.base_url
        ))
        .header("cookie", cookies.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .header("idempotency-key", "http-group-member")
        .json(&serde_json::json!({"present":true,"expected_revision":0}))
        .send()
        .await
        .unwrap();
    assert_eq!(member.status(), 200);
    let app = first["application_id"].as_i64().unwrap();
    let assigned=f.client.patch(format!("{}/api/v1/admin/apps/{app}/assignments",f.base_url)).header("cookie",cookies.header()).header("origin",&f.base_url).header("x-csrf-token",&csrf).header("idempotency-key","http-group-assign").json(&serde_json::json!({"target":{"kind":"group","id":id},"present":true,"expected_revision":1})).send().await.unwrap();
    assert_eq!(assigned.status(), 200);
    let groups = f
        .client
        .get(format!("{}/api/v1/admin/groups?q=HTTP", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(groups.status(), 200);
    let groups: serde_json::Value = groups.json().await.unwrap();
    assert_eq!(groups["items"].as_array().unwrap().len(), 1);
    assert_eq!(groups["items"][0]["name"], "HTTP group");
    assert_eq!(groups["items"][0]["member_profiles"][0]["user_id"], owner);
    let group_by_id = f
        .client
        .get(format!("{}/api/v1/admin/groups?q={id}", f.base_url))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(group_by_id.status(), 200);
    let group_by_id: serde_json::Value = group_by_id.json().await.unwrap();
    assert_eq!(group_by_id["items"].as_array().unwrap().len(), 1);
    assert_eq!(group_by_id["items"][0]["group_id"], id);
    let assignments = f
        .client
        .get(format!(
            "{}/api/v1/admin/apps/{app}/assignments",
            f.base_url
        ))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(assignments.status(), 200);
    let assignments: serde_json::Value = assignments.json().await.unwrap();
    let items = assignments.as_array().unwrap();
    assert_eq!(items.len(), 2);
    let group_entry = items
        .iter()
        .find(|item| item["label"] == "HTTP group")
        .unwrap();
    assert!(group_entry["secondary"].as_str().unwrap().contains(id));
    let missing=f.client.patch(format!("{}/api/v1/admin/apps/{app}/assignments",f.base_url)).header("cookie",cookies.header()).header("origin",&f.base_url).header("idempotency-key","missing-csrf").json(&serde_json::json!({"target":{"kind":"user","id":owner},"present":false,"expected_revision":2})).send().await.unwrap();
    assert_eq!(missing.status(), 403);
    f.close().await;
}
