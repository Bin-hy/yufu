//! Assignment sources, generation changes and concurrent admission scenarios.
use super::super::common::IdentityFixture;
use super::identity_support::*;
use uuid::Uuid;
use yufu::domain::identity::{application::IdentityScopeSet, error::IdentityError};
use yufu::service::identity::repository::*;

#[tokio::test]
async fn identity_assignment_overlapping_sources_preserve_generation_until_final_loss() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "assigned", false).await;
    let root = center(&f, a.owner).await;
    let group = Uuid::new_v4();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::CreateGroup {
        id: group,
        name: "source-group".into(),
        description: None,
        actor_id: a.owner,
    })
    .await
    .unwrap();
    tx.apply(IdentityWrite::ChangeGroupMember {
        group_id: group,
        user_id: a.owner,
        present: true,
        expected_revision: 0,
        actor_id: a.owner,
    })
    .await
    .unwrap();
    for target in [
        AssignmentTarget::User(a.owner),
        AssignmentTarget::Group(group),
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
    for environment_id in [a.development, a.production] {
        tx.apply(IdentityWrite::RecomputeGeneration {
            environment_id,
            user_id: a.owner,
        })
        .await
        .unwrap();
    }
    let query = || AdmissionQuery {
        sso_session_id: root.id,
        environment_id: a.production,
        requested_scopes: IdentityScopeSet::parse("openid").unwrap(),
    };
    let old = tx.evaluate_admission(&query()).await.unwrap();
    assert_eq!(old.access_generation(), 0);
    tx.commit().await.unwrap();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::SetAssignment {
        application_id: a.app,
        target: AssignmentTarget::User(a.owner),
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
    let retained = tx.evaluate_admission(&query()).await.unwrap();
    assert!(old.still_current(&retained));
    tx.commit().await.unwrap();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::SetAssignment {
        application_id: a.app,
        target: AssignmentTarget::Group(group),
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
    assert_eq!(
        tx.evaluate_admission(&query()).await,
        Err(IdentityError::AccessDenied)
    );
    tx.commit().await.unwrap();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::SetAssignment {
        application_id: a.app,
        target: AssignmentTarget::User(a.owner),
        present: true,
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
    let new = tx.evaluate_admission(&query()).await.unwrap();
    assert_eq!(new.access_generation(), 1);
    assert!(!old.still_current(&new));
    let dev = tx
        .evaluate_admission(&AdmissionQuery {
            sso_session_id: root.id,
            environment_id: a.development,
            requested_scopes: IdentityScopeSet::parse("openid").unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(dev.access_generation(), 0);
    tx.commit().await.unwrap();
    f.close().await;
}
