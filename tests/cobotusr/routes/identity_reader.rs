//! Real primary transactions for reader idempotency and enterprise authority.
use super::super::common::IdentityFixture;
use super::identity_support::*;
use cobotusr::{
    domain::identity::{application::ClientKind, error::IdentityError},
    service::identity::reader::{ReaderLocation, ReaderManagement, ReaderService},
};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn identity_protocol_reader_idempotency_returns_only_safe_receipts_after_lost_response() {
    let f = IdentityFixture::start().await;
    let a = app_kind(&f, "idempotent-reader", false, ClientKind::BrowserSpa).await;
    let (_, root) = center_secret(&f, a.owner).await;
    let browser = cobotusr::domain::identity::account::IssuedSecret::from_entropy(rand::random())
        .into_response();
    let evidence = write_evidence(&f, Some(&root), &browser);
    let service = ReaderService::new(Arc::new(repository(&f)), f.clock.clone());
    let location = ReaderLocation {
        application_id: a.app,
        environment_id: a.development,
    };
    let first_budget = budget(&f);
    let second_budget = budget(&f);
    let (first, second) = tokio::join!(
        service.create(
            &evidence,
            location,
            "urn:test:Development",
            "reader-once",
            Uuid::new_v4(),
            &first_budget
        ),
        service.create(
            &evidence,
            location,
            "urn:test:Development",
            "reader-once",
            Uuid::new_v4(),
            &second_budget
        )
    );
    let first = first.unwrap();
    let second = second.unwrap();
    assert_eq!(first.operation_id, second.operation_id);
    assert_eq!(first.id, second.id);
    assert_ne!(first.secret.is_some(), second.secret.is_some());
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_introspection_reader"
        )
        .await,
        1
    );
    let replay = service
        .create(
            &evidence,
            location,
            "urn:test:Development",
            "reader-once",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(replay.secret.is_none());
    assert_eq!(replay.operation_id, first.operation_id);
    assert_eq!(
        service
            .create(
                &evidence,
                location,
                "urn:changed",
                "reader-once",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::Conflict)
    );
    let changed = service
        .change(
            &evidence,
            &first.reader_id,
            location,
            1,
            true,
            true,
            "reset-once",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(changed.secret.is_some());
    assert_eq!(changed.revision, 2);
    let replay = service
        .change(
            &evidence,
            &first.reader_id,
            location,
            1,
            true,
            true,
            "reset-once",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(replay.secret.is_none());
    assert_eq!(replay.revision, 2);
    assert_eq!(replay.operation_id, changed.operation_id);
    assert_eq!(
        service
            .change(
                &evidence,
                &first.reader_id,
                location,
                1,
                false,
                true,
                "reset-once",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::Conflict)
    );
    let receipt = cobotusr::service::identity::application::ApplicationService::new(
        Arc::new(repository(&f)),
        f.clock.clone(),
    )
    .operation(&root, changed.operation_id, &budget(&f))
    .await
    .unwrap();
    let safe = serde_json::to_string(&receipt).unwrap();
    assert!(!safe.contains("secret"));
    assert!(!safe.contains("hash") || safe.contains("request_hash"));
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action LIKE 'reader.%'").await,2);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_request_operation WHERE operation_kind LIKE 'reader.%'").await,2);
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_reader_allows_confidential_web_for_its_own_audience_only() {
    let f = IdentityFixture::start().await;
    let web = app_kind(&f, "reader-web", false, ClientKind::ServerWeb).await;
    let other = app_kind(&f, "reader-web-other", false, ClientKind::ServerWeb).await;
    let (_, root) = center_secret(&f, web.owner).await;
    let browser = cobotusr::domain::identity::account::IssuedSecret::from_entropy(rand::random())
        .into_response();
    let evidence = write_evidence(&f, Some(&root), &browser);
    let service = ReaderService::new(Arc::new(repository(&f)), f.clock.clone());
    let location = ReaderLocation {
        application_id: web.app,
        environment_id: web.development,
    };

    let issued = service
        .create(
            &evidence,
            location,
            "urn:test:Development",
            "web-reader",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(issued.secret.is_some());

    assert_eq!(
        service
            .create(
                &evidence,
                location,
                "urn:unapproved",
                "web-reader-wrong-audience",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::AccessDenied)
    );
    assert_eq!(
        service
            .create(
                &evidence,
                ReaderLocation {
                    application_id: web.app,
                    environment_id: other.development,
                },
                "urn:test:Development",
                "web-reader-other-environment",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::Forbidden)
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_introspection_reader"
        )
        .await,
        1
    );
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_reader_administrator_requires_exact_operation_without_membership() {
    let f = IdentityFixture::start().await;
    let a = app_kind(&f, "admin-reader", false, ClientKind::BrowserSpa).await;
    let actor = admin_id(&f).await;
    let row=f.db.query_one_raw(Statement::from_string(DbBackend::Postgres,"INSERT INTO \"user\" (username,password,role_id,create_time,update_time) VALUES ('reader-owner','unused',1,NOW(),NOW()) RETURNING id")).await.unwrap().unwrap();
    let owner: i32 = row.try_get("", "id").unwrap();
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE identity_app_member SET user_id=$2 WHERE application_id=$1 AND member_role='owner'",
        [a.app.into(), owner.into()],
    ))
    .await
    .unwrap();
    let (_, root) = center_secret(&f, actor).await;
    let browser = cobotusr::domain::identity::account::IssuedSecret::from_entropy(rand::random())
        .into_response();
    let evidence = write_evidence(&f, Some(&root), &browser);
    let location = ReaderLocation {
        application_id: a.app,
        environment_id: a.development,
    };
    let developer = ReaderService::new(Arc::new(repository(&f)), f.clock.clone());
    assert_eq!(
        developer
            .create(
                &evidence,
                location,
                "urn:test:Development",
                "dev-denied",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::Forbidden)
    );
    let service = ReaderService::new(Arc::new(repository(&f)), f.clock.clone())
        .with_management(ReaderManagement::Administrator);
    let issued = service
        .create(
            &evidence,
            location,
            "urn:test:Development",
            "admin-reader",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .list(&root, location, None, 25, &budget(&f))
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) SELECT $1,id,FALSE,NOW(),NOW() FROM authority WHERE method='PATCH' AND authority_uri='/admin/apps/{id}/environments/{env}/integration/readers/{reader_id}'",[actor.into()])).await.unwrap();
    assert_eq!(
        service
            .change(
                &evidence,
                &issued.reader_id,
                location,
                1,
                false,
                false,
                "admin-denied",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::Forbidden)
    );
    // Read permission remains independent of the denied write registration.
    assert_eq!(
        service
            .list(&root, location, None, 25, &budget(&f))
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    f.db.execute_unprepared("DELETE FROM authority_role WHERE authority_id IN (SELECT id FROM authority WHERE method='POST' AND authority_uri='/admin/apps/{id}/environments/{env}/integration/readers'); DELETE FROM authority WHERE method='POST' AND authority_uri='/admin/apps/{id}/environments/{env}/integration/readers'").await.unwrap();
    assert_eq!(
        service
            .create(
                &evidence,
                location,
                "urn:test:Development",
                "unregistered",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::Forbidden)
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_introspection_reader WHERE enabled"
        )
        .await,
        1
    );
    f.close().await;
}

#[tokio::test]
async fn identity_protocol_reader_commit_failure_preserves_revision_secret_and_receipt() {
    let f = IdentityFixture::start().await;
    let a = app_kind(&f, "reader-commit", false, ClientKind::BrowserSpa).await;
    let (_, root) = center_secret(&f, a.owner).await;
    let browser = cobotusr::domain::identity::account::IssuedSecret::from_entropy(rand::random())
        .into_response();
    let evidence = write_evidence(&f, Some(&root), &browser);
    let service = ReaderService::new(Arc::new(repository(&f)), f.clock.clone());
    let location = ReaderLocation {
        application_id: a.app,
        environment_id: a.development,
    };
    let issued = service
        .create(
            &evidence,
            location,
            "urn:test:Development",
            "before-failure",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    f.db.execute_unprepared("CREATE FUNCTION reject_reader_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture commit rejection'; END; $$; CREATE CONSTRAINT TRIGGER reject_reader_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_reader_commit()").await.unwrap();
    let result = service
        .change(
            &evidence,
            &issued.reader_id,
            location,
            1,
            true,
            true,
            "failed-reset",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await;
    assert!(result.is_err());
    assert_eq!(
        scalar(
            &f,
            "SELECT MAX(secret_revision) AS value FROM identity_introspection_reader"
        )
        .await,
        1
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_request_operation WHERE operation_kind='reader.secret.reset'").await,0);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='reader.secret.reset'").await,0);
    f.db.execute_unprepared("DROP TRIGGER reject_reader_commit ON identity_audit_event; DROP FUNCTION reject_reader_commit()").await.unwrap();
    let result = service
        .change(
            &evidence,
            &issued.reader_id,
            location,
            1,
            true,
            true,
            "failed-reset",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(result.secret.is_some());
    assert_eq!(result.revision, 2);
    f.close().await;
}
