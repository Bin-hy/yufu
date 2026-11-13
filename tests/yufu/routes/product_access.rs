use super::{
    super::common::IdentityFixture,
    identity_browser::{BrowserCookies, http_login},
    identity_support::*,
};
use yufu::domain::identity::application::{AppEnvironmentKind, ClientKind};

#[tokio::test]
async fn product_launch_http_rechecks_admission_and_uses_active_version() {
    let f = IdentityFixture::start().await;
    let development = config_with(
        AppEnvironmentKind::Development,
        ClientKind::ServerWeb,
        false,
        |c| {
            c.launch_url = Some("https://rp.example/home?tab=work".into());
            c.launch_description = Some("Workbench".into());
        },
    );
    let a = app_with_configs(
        &f,
        "product-launch",
        ClientKind::ServerWeb,
        development,
        config(AppEnvironmentKind::Production, ClientKind::ServerWeb, false),
    )
    .await;
    // A saved proposal must not replace the entry point of the active version.
    {
        use yufu::service::identity::repository::{
            IdentityRepository, IdentityWrite, VersionWrite,
        };
        let pending = config_with(
            AppEnvironmentKind::Development,
            ClientKind::ServerWeb,
            false,
            |c| {
                c.launch_url = Some("https://rp.example/not-active".into());
                c.launch_description = Some("Unapproved proposal".into());
            },
        );
        let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
        tx.apply(IdentityWrite::InsertVersion(Box::new(VersionWrite {
            id: uuid::Uuid::new_v4(),
            environment_id: a.development,
            expected_revision: 0,
            config: pending,
            candidate_client_id: None,
            actor_id: a.owner,
        })))
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    let url = format!(
        "{}/api/v1/account/apps/{}/launch",
        f.base_url, a.development
    );
    assert_eq!(f.client.get(&url).send().await.unwrap().status(), 401);
    let mut cookies = BrowserCookies::default();
    http_login(&f, &mut cookies).await;
    let listed: serde_json::Value = f
        .client
        .get(format!(
            "{}/api/v1/account/apps?environment=development&q=product-launch",
            f.base_url
        ))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);
    assert_eq!(listed["items"][0]["launchable"], true);
    assert_eq!(listed["items"][0]["launch_description"], "Workbench");
    assert_eq!(listed["items"][0]["sources"][0]["kind"], "maintainer");
    let production: serde_json::Value = f
        .client
        .get(format!(
            "{}/api/v1/account/apps?environment=production",
            f.base_url
        ))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(production["items"].as_array().unwrap().is_empty());
    let unmatched: serde_json::Value = f
        .client
        .get(format!(
            "{}/api/v1/account/apps?environment=development&q=unmatched",
            f.base_url
        ))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(unmatched["items"].as_array().unwrap().is_empty());
    let response = f
        .client
        .get(&url)
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 303);
    assert_eq!(
        response.headers()["location"],
        "https://rp.example/home?tab=work"
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
    let injected = f
        .client
        .get(format!("{url}?return_to=https://evil.example/"))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert_eq!(injected.status(), 400);
    let denied = f
        .client
        .get(format!(
            "{}/api/v1/account/apps/{}/launch",
            f.base_url, a.production
        ))
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap();
    assert!(!denied.status().is_redirection());
    assert!(!denied.status().is_success());
}

#[tokio::test]
async fn product_launch_revocation_preserves_other_sources_and_rejects_last_removal() {
    use uuid::Uuid;
    use yufu::service::identity::repository::{
        AssignmentTarget, IdentityRepository, IdentityWrite,
    };
    let f = IdentityFixture::start().await;
    let production = config_with(
        AppEnvironmentKind::Production,
        ClientKind::ServerWeb,
        false,
        |c| {
            c.launch_url = Some("https://rp.example/production".into());
        },
    );
    let a = app_with_configs(
        &f,
        "product-launch-sources",
        ClientKind::ServerWeb,
        config(
            AppEnvironmentKind::Development,
            ClientKind::ServerWeb,
            false,
        ),
        production,
    )
    .await;
    let group_id = Uuid::new_v4();
    let repo = repository(&f);
    let mut tx = repo.begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::CreateGroup {
        id: group_id,
        name: "Research".into(),
        description: None,
        actor_id: a.owner,
    })
    .await
    .unwrap();
    tx.apply(IdentityWrite::ChangeGroupMember {
        group_id,
        user_id: a.owner,
        present: true,
        expected_revision: 0,
        actor_id: a.owner,
    })
    .await
    .unwrap();
    for target in [
        AssignmentTarget::User(a.owner),
        AssignmentTarget::Group(group_id),
    ] {
        tx.apply(IdentityWrite::SetAssignment {
            application_id: a.app,
            target,
            present: true,
            actor_id: a.owner,
        })
        .await
        .unwrap();
    }
    let snapshot = tx
        .approval_snapshot(a.production, a.prod_version, false)
        .await
        .unwrap();
    tx.apply(IdentityWrite::RecordAdminActivation {
        snapshot,
        actor_id: a.owner,
    })
    .await
    .unwrap();
    tx.apply(IdentityWrite::ActivateVersion {
        environment_id: a.production,
        version_id: a.prod_version,
        expected_revision: 0,
    })
    .await
    .unwrap();
    tx.apply(IdentityWrite::RecomputeGeneration {
        environment_id: a.production,
        user_id: a.owner,
    })
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let mut cookies = BrowserCookies::default();
    http_login(&f, &mut cookies).await;
    let list_url = format!("{}/api/v1/account/apps?environment=production", f.base_url);
    let launch_url = format!("{}/api/v1/account/apps/{}/launch", f.base_url, a.production);
    let list: serde_json::Value = f
        .client
        .get(&list_url)
        .header("cookie", cookies.header())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list["items"][0]["sources"].as_array().unwrap().len(), 2);
    assert_eq!(list["items"][0]["sources"][1]["label"], "Research");
    for (target, expected) in [
        (AssignmentTarget::User(a.owner), 303),
        (AssignmentTarget::Group(group_id), 403),
    ] {
        let mut tx = repo.begin(&budget(&f)).await.unwrap();
        tx.apply(IdentityWrite::SetAssignment {
            application_id: a.app,
            target,
            present: false,
            actor_id: a.owner,
        })
        .await
        .unwrap();
        tx.apply(IdentityWrite::RecomputeGeneration {
            environment_id: a.production,
            user_id: a.owner,
        })
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let response = f
            .client
            .get(&launch_url)
            .header("cookie", cookies.header())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        if expected == 403 {
            assert!(response.headers().get("location").is_none());
        }
    }
}
