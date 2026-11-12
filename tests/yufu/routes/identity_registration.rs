//! Self-service application registration through the authenticated service boundary.
use super::super::common::IdentityFixture;
use super::identity_support::*;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use std::sync::Arc;
use uuid::Uuid;
use yufu::domain::identity::{
    application::{AppEnvironmentKind, ClientKind},
    error::IdentityError,
    registration::CreateApplication,
};
use yufu::service::identity::application::ApplicationService;

fn request(key: &str, kind: ClientKind) -> CreateApplication {
    CreateApplication {
        app_key: key.into(),
        name: "My enterprise app".into(),
        description: None,
        third_party: false,
        development: config(AppEnvironmentKind::Development, kind, false).to_input(),
        production: config(AppEnvironmentKind::Production, kind, false).to_input(),
    }
}
fn service(f: &IdentityFixture) -> ApplicationService {
    ApplicationService::new(Arc::new(repository(f)), f.clock.clone())
}
async fn developer(f: &IdentityFixture) -> (i32, String) {
    let id = admin_id(f).await;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET developer_enabled=true WHERE id=$1",
        [id.into()],
    ))
    .await
    .unwrap();
    let (_, secret) = center_secret(f, id).await;
    (id, secret)
}

#[tokio::test]
async fn identity_application_service_registration_builds_two_isolated_environments_with_one_time_secrets()
 {
    let f = IdentityFixture::start().await;
    let (id, center) = developer(&f).await;
    let evidence = write_evidence(&f, Some(&center), &"A".repeat(43));
    let svc = service(&f);
    let first = svc
        .register(
            &evidence,
            request("self-service", ClientKind::ServerWeb),
            "register-once",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(first.environments.len(), 2);
    assert_eq!(first.newly_issued_secrets.len(), 2);
    assert_ne!(
        first.environments[0].client_id,
        first.environments[1].client_id
    );
    assert!(first.environments[0].active_version_id.is_some());
    assert_eq!(first.environments[1].active_version_id, None);
    let mut values = Vec::new();
    for (env, secret) in first.newly_issued_secrets {
        let value = secret.into_response();
        let row = f.db.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "SELECT c.client_secret_hash,c.user_id,t.enabled FROM identity_app_environment e JOIN oauth_client c ON c.id=e.oauth_client_id JOIN security_session_client t ON t.client_id=c.client_id WHERE e.id=$1", [env.into()])).await.unwrap().unwrap();
        assert!(
            bcrypt::verify(
                &value,
                &row.try_get::<String>("", "client_secret_hash").unwrap()
            )
            .unwrap()
        );
        assert_eq!(row.try_get::<i32>("", "user_id").unwrap(), id);
        assert_eq!(
            row.try_get::<bool>("", "enabled").unwrap(),
            env == first.environments[0].environment_id
        );
        values.push(value);
    }
    assert_ne!(values[0], values[1]);
    let replay = svc
        .register(
            &evidence,
            request("self-service", ClientKind::ServerWeb),
            "register-once",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(replay.application_id, first.application_id);
    assert_eq!(replay.operation_id, first.operation_id);
    assert!(replay.newly_issued_secrets.is_empty());
    assert_eq!(
        svc.register(
            &evidence,
            request("another-request", ClientKind::ServerWeb),
            "register-once",
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .err(),
        Some(IdentityError::Conflict)
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM application WHERE management_state='managed'"
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_app_assignment"
        )
        .await,
        0
    );
    assert_eq!(scalar(&f, "SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='application.register'").await, 1);
    f.close().await;
}

#[tokio::test]
async fn identity_application_service_registration_requires_developer_and_spa_never_gets_a_secret()
{
    let f = IdentityFixture::start().await;
    let id = admin_id(&f).await;
    let (_, center) = center_secret(&f, id).await;
    let evidence = write_evidence(&f, Some(&center), &"A".repeat(43));
    let svc = service(&f);
    assert_eq!(
        svc.register(
            &evidence,
            request("admin-is-not-developer", ClientKind::BrowserSpa),
            "denied",
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .err(),
        Some(IdentityError::Forbidden)
    );
    let (_, center) = developer(&f).await;
    let evidence = write_evidence(&f, Some(&center), &"A".repeat(43));
    let receipt = svc
        .register(
            &evidence,
            request("public-spa", ClientKind::BrowserSpa),
            "spa",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(receipt.newly_issued_secrets.is_empty());
    assert_eq!(scalar(&f, "SELECT COUNT(*)::bigint AS value FROM oauth_client WHERE management_state='managed' AND public_client AND client_secret_hash='' AND grant_types NOT LIKE '%client_credentials%'").await, 2);
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET enabled=false WHERE id=$1",
        [id.into()],
    ))
    .await
    .unwrap();
    assert!(
        svc.register(
            &evidence,
            request("public-spa", ClientKind::BrowserSpa),
            "spa",
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_err()
    );
    f.close().await;
}

#[tokio::test]
async fn identity_application_service_registration_concurrent_replay_and_commit_failure_leave_no_half_application()
 {
    let f = IdentityFixture::start().await;
    let (_, center) = developer(&f).await;
    let evidence = write_evidence(&f, Some(&center), &"A".repeat(43));
    let svc = service(&f);
    let one_budget = budget(&f);
    let two_budget = budget(&f);
    let (one, two) = tokio::join!(
        svc.register(
            &evidence,
            request("parallel-spa", ClientKind::BrowserSpa),
            "parallel",
            Uuid::new_v4(),
            &one_budget
        ),
        svc.register(
            &evidence,
            request("parallel-spa", ClientKind::BrowserSpa),
            "parallel",
            Uuid::new_v4(),
            &two_budget
        )
    );
    let one = one.unwrap();
    let two = two.unwrap();
    assert_eq!(one.application_id, two.application_id);
    assert_eq!(one.operation_id, two.operation_id);
    f.db.execute_unprepared("CREATE FUNCTION reject_registration_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='application.register' THEN RAISE EXCEPTION 'fixture rejected commit'; END IF; RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER reject_registration_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_registration_commit()").await.unwrap();
    assert_eq!(
        svc.register(
            &evidence,
            request("failed-registration", ClientKind::ServerWeb),
            "failed",
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .err(),
        Some(IdentityError::DependencyUnavailable)
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM application WHERE management_state='managed'"
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM oauth_client WHERE management_state='managed'"
        )
        .await,
        2
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_request_operation"
        )
        .await,
        1
    );
    f.close().await;
}
