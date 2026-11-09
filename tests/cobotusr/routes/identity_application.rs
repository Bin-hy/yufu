//! Controlled registration, membership and immutable approval scenarios.
use super::super::common::IdentityFixture;
use super::identity_support::*;
use cobotusr::domain::identity::{
    application::{AppEnvironmentKind, ClientKind, IdentityScopeSet},
    error::IdentityError,
};
use cobotusr::service::identity::repository::*;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use uuid::Uuid;

#[tokio::test]
async fn identity_application_registration_keeps_production_disabled_and_bounds_empty() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "registered", false).await;
    let root = center(&f, a.owner).await;
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    for environment_id in [a.development, a.production] {
        tx.apply(IdentityWrite::RecomputeGeneration {
            environment_id,
            user_id: a.owner,
        })
        .await
        .unwrap();
    }
    let dev = tx
        .evaluate_admission(&AdmissionQuery {
            sso_session_id: root.id,
            environment_id: a.development,
            requested_scopes: IdentityScopeSet::parse("openid email").unwrap(),
        })
        .await
        .unwrap();
    assert!(dev.session_bounds().grants().is_empty());
    assert_eq!(dev.config_version_id(), a.dev_version);
    assert_eq!(
        tx.evaluate_admission(&AdmissionQuery {
            sso_session_id: root.id,
            environment_id: a.production,
            requested_scopes: IdentityScopeSet::parse("openid").unwrap()
        })
        .await,
        Err(IdentityError::AccessDenied)
    );
    tx.commit().await.unwrap();
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
            "SELECT COUNT(*)::bigint AS value FROM security_session_client WHERE enabled"
        )
        .await,
        1
    );
    f.close().await;
}

#[tokio::test]
async fn identity_application_new_version_keeps_active_pointer_until_activation_and_cannot_be_overwritten()
 {
    let f = IdentityFixture::start().await;
    let a = app(&f, "versioned", false).await;
    let next = Uuid::new_v4();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::InsertVersion(Box::new(VersionWrite {
        id: next,
        environment_id: a.development,
        expected_revision: 0,
        config: config(
            AppEnvironmentKind::Development,
            ClientKind::ServerWeb,
            false,
        ),
        candidate_client_id: None,
        actor_id: a.owner,
    })))
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let active =
        f.db.query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT active_version_id FROM identity_app_environment WHERE id=$1",
            [a.development.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Uuid>("", "active_version_id")
        .unwrap();
    assert_eq!(active, a.dev_version);
    assert!(
        f.db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE identity_app_config_version SET payload='{}'::jsonb WHERE id=$1",
            [next.into()]
        ))
        .await
        .is_err()
    );
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::ActivateVersion {
        environment_id: a.development,
        version_id: next,
        expected_revision: 1,
    })
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        scalar(
            &f,
            "SELECT SUM(lifecycle_epoch)::bigint AS value FROM identity_app_environment"
        )
        .await,
        1
    );
    f.close().await;
}

#[tokio::test]
async fn identity_application_approval_rejects_self_review_and_obsolete_snapshot() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "approval", false).await;
    let approval = Uuid::new_v4();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let snapshot = tx
        .approval_snapshot(a.production, a.prod_version, false)
        .await
        .unwrap();
    tx.apply(IdentityWrite::SubmitApproval(Box::new(ApprovalWrite {
        id: approval,
        snapshot,
        requested_by: a.owner,
    })))
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    assert_eq!(
        tx.apply(IdentityWrite::ReviewApproval {
            approval_id: approval,
            reviewer_id: a.owner,
            approve: true,
            reason: None,
            accept_logout_limitation: false
        })
        .await
        .err(),
        Some(IdentityError::SelfApproval)
    );
    tx.rollback().await.unwrap();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::SetAssignment {
        application_id: a.app,
        target: AssignmentTarget::User(a.owner),
        present: true,
        actor_id: a.owner,
    })
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_app_approval WHERE status='obsolete'"
        )
        .await,
        1
    );
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    assert!(
        tx.apply(IdentityWrite::ActivateVersion {
            environment_id: a.production,
            version_id: a.prod_version,
            expected_revision: 0
        })
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    f.close().await;
}

#[tokio::test]
async fn identity_application_receipt_is_safe_and_key_with_different_hash_cannot_be_reused() {
    let f = IdentityFixture::start().await;
    let actor = admin_id(&f).await;
    let id = Uuid::new_v4();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::SaveOperation(OperationReceiptWrite {
        id,
        actor_id: actor,
        operation_kind: "register",
        idempotency_key: "same".into(),
        request_hash: "a".repeat(64),
        resource_id: "7".into(),
        summary: serde_json::json!({"application_id":7,"development_client_id":"web-dev"}),
    }))
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let receipt = tx
        .operation_receipt(actor, "register", "same")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.operation_id, id);
    assert_eq!(receipt.request_hash, "a".repeat(64));
    assert!(receipt.summary.get("secret").is_none());
    assert_eq!(
        tx.apply(IdentityWrite::SaveOperation(OperationReceiptWrite {
            id: Uuid::new_v4(),
            actor_id: actor,
            operation_kind: "register",
            idempotency_key: "same".into(),
            request_hash: "b".repeat(64),
            resource_id: "8".into(),
            summary: serde_json::json!({})
        }))
        .await
        .err(),
        Some(IdentityError::Conflict)
    );
    tx.rollback().await.unwrap();
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
