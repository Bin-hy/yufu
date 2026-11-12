//! Application governance uses the real primary transaction, not direct route CRUD.
use super::{super::common::IdentityFixture, identity_support::*};
use auth::security::identity::SecretToken;
use cobotusr::{
    domain::identity::{
        account::{IssuedSecret, PersonKind},
        application::{AppEnvironmentKind, ClientKind, LogoutCapability},
        error::IdentityError,
    },
    service::identity::{
        application::{ApplicationMutation, ApplicationService},
        repository::*,
    },
};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use std::sync::Arc;
use uuid::Uuid;

fn service(f: &IdentityFixture) -> ApplicationService {
    ApplicationService::new(Arc::new(repository(f)), f.clock.clone())
}
async fn evidence(
    f: &IdentityFixture,
    user: i32,
) -> cobotusr::service::identity::browser::BrowserWriteEvidence {
    let (_, secret) = center_secret(f, user).await;
    write_evidence(
        f,
        Some(&secret),
        &IssuedSecret::from_entropy([9; 32]).into_response(),
    )
}
async fn developer_managed(f: &IdentityFixture, a: &AppSeed) {
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE application SET management_mode='developer_managed' WHERE id=$1",
        [a.app.into()],
    ))
    .await
    .unwrap();
}
async fn second_developer(f: &IdentityFixture, administrator: bool) -> i32 {
    let mut tx = repository(f).begin(&budget(f)).await.unwrap();
    let IdentityWriteResult::Account(user) = tx
        .apply(IdentityWrite::CreateAccount(NewAccountWrite {
            username: "governance-second".into(),
            nickname: "Second".into(),
            password_hash: SecretToken::new("fixture-unused-password".into()).unwrap(),
            person_kind: PersonKind::Collaborator,
            developer_enabled: true,
        }))
        .await
        .unwrap()
    else {
        panic!("account expected")
    };
    tx.commit().await.unwrap();
    if administrator {
        f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "UPDATE \"user\" SET role_id=(SELECT role_id FROM \"user\" WHERE username='identity-admin') WHERE id=$1", [user.user_id.into()])).await.unwrap();
    }
    user.user_id
}

#[tokio::test]
async fn identity_application_governance_development_switch_is_atomic_and_type_change_uses_a_new_client()
 {
    let f = IdentityFixture::start().await;
    let a = app(&f, "governance-dev", true).await;
    developer_managed(&f, &a).await;
    let root = center(&f, a.owner).await;
    let (_, _, rp) = identity_tokens(&f, &a, &root, "openid").await;
    let evidence = evidence(&f, a.owner).await;
    let service = service(&f);
    let change = ApplicationMutation::UpdateConfig {
        environment_id: a.development,
        expected_revision: 0,
        config: config(
            AppEnvironmentKind::Development,
            ClientKind::BrowserSpa,
            true,
        )
        .to_input(),
    };
    let result = service
        .mutate(
            &evidence,
            a.app,
            change.clone(),
            "dev-change",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(result.newly_issued_secret.is_none());
    assert_ne!(result.receipt.summary["client_id"], "governance-dev-dev");
    assert_eq!(result.receipt.summary["revision"], 2);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE revoked_at IS NOT NULL").await,1);
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery WHERE rp_session_id='{rp}'")).await,1);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM security_session_client WHERE enabled"
        )
        .await,
        1
    );
    let repeat = service
        .mutate(
            &evidence,
            a.app,
            change,
            "dev-change",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(repeat.receipt.operation_id, result.receipt.operation_id);
    assert!(repeat.newly_issued_secret.is_none());
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_app_config_version"
        )
        .await,
        3
    );
    let reset = service
        .mutate(
            &evidence,
            a.app,
            ApplicationMutation::ResetSecret {
                environment_id: a.development,
                expected_revision: 2,
            },
            "no-spa-secret",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .err();
    assert_eq!(reset, Some(IdentityError::InvalidInput));
    let foreign = app(&f, "governance-foreign", false).await;
    assert_eq!(
        service
            .mutate(
                &evidence,
                a.app,
                ApplicationMutation::UpdateConfig {
                    environment_id: foreign.development,
                    expected_revision: 0,
                    config: config(
                        AppEnvironmentKind::Development,
                        ClientKind::ServerWeb,
                        false
                    )
                    .to_input(),
                },
                "wrong-env",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::NotFound)
    );
    f.close().await;
}

#[tokio::test]
async fn identity_application_governance_production_review_requires_independence_current_snapshot_and_explicit_admin_acceptance()
 {
    let f = IdentityFixture::start().await;
    let a = app(&f, "governance-prod", false).await;
    developer_managed(&f, &a).await;
    let owner = evidence(&f, a.owner).await;
    let reviewer_id = second_developer(&f, true).await;
    let reviewer = evidence(&f, reviewer_id).await;
    let service = service(&f);
    let mut config = config(
        AppEnvironmentKind::Production,
        ClientKind::BrowserSpa,
        false,
    )
    .to_input();
    config.logout_capability = LogoutCapability::LocalOnly;
    let draft = service
        .mutate(
            &owner,
            a.app,
            ApplicationMutation::UpdateConfig {
                environment_id: a.production,
                expected_revision: 0,
                config,
            },
            "prod-draft",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    let version = serde_json::from_value(draft.receipt.summary["version_id"].clone()).unwrap();
    let submitted = service
        .mutate(
            &owner,
            a.app,
            ApplicationMutation::SubmitProduction {
                environment_id: a.production,
                expected_revision: 1,
                version_id: version,
            },
            "prod-submit",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    let approval: Uuid =
        serde_json::from_value(submitted.receipt.summary["approval_id"].clone()).unwrap();
    assert_eq!(submitted.receipt.summary["revision"], 2);
    let review = ApplicationMutation::ReviewProduction {
        approval_id: approval,
        expected_revision: 2,
        approve: true,
        reason: None,
        accept_logout_limitation: false,
    };
    assert_eq!(
        service
            .mutate(
                &owner,
                a.app,
                review.clone(),
                "self-review",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::SelfApproval)
    );
    assert_eq!(
        service
            .mutate(
                &reviewer,
                a.app,
                review,
                "no-accept",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::InvalidInput)
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_app_environment WHERE kind='production' AND active_version_id IS NOT NULL").await,0);
    let approved = service
        .mutate(
            &reviewer,
            a.app,
            ApplicationMutation::ReviewProduction {
                approval_id: approval,
                expected_revision: 2,
                approve: true,
                reason: Some("Reviewed exact callbacks".into()),
                accept_logout_limitation: true,
            },
            "approved",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(approved.receipt.summary["revision"], 3);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_app_approval WHERE status='approved' AND logout_limitation_accepted").await,1);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_app_assignment"
        )
        .await,
        0
    );
    let snapshot =
        f.db.query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT snapshot FROM identity_app_approval WHERE id=$1",
            [approval.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<serde_json::Value>("", "snapshot")
        .unwrap();
    assert_eq!(snapshot["logout_limitation_accepted"], false);
    assert!(
        f.db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE identity_app_approval SET snapshot='{}'::jsonb WHERE id=$1",
            [approval.into()]
        ))
        .await
        .is_err()
    );
    f.close().await;
}

#[tokio::test]
async fn identity_application_governance_secret_reset_is_single_environment_one_time_and_commit_failure_preserves_state()
 {
    let f = IdentityFixture::start().await;
    let a = app(&f, "governance-reset", false).await;
    developer_managed(&f, &a).await;
    let owner = evidence(&f, a.owner).await;
    let service = service(&f);
    let change = ApplicationMutation::ResetSecret {
        environment_id: a.development,
        expected_revision: 0,
    };
    let first = service
        .mutate(
            &owner,
            a.app,
            change.clone(),
            "reset-once",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    let secret = first.newly_issued_secret.unwrap().into_response();
    let hash =
        f.db.query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT client_secret_hash FROM oauth_client WHERE client_id='governance-reset-dev'",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<String>("", "client_secret_hash")
        .unwrap();
    assert!(bcrypt::verify(&secret, &hash).unwrap());
    assert!(!bcrypt::verify("fixture-client-secret", &hash).unwrap());
    assert_eq!(first.receipt.summary["secret_revision"], 1);
    assert_eq!(scalar(&f,"SELECT secret_revision AS value FROM oauth_client WHERE client_id='governance-reset-dev'").await, 1);
    assert_eq!(scalar(&f,"SELECT secret_revision AS value FROM oauth_client WHERE client_id='governance-reset-prod'").await, 0);
    let production_hash: String =
        f.db.query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT client_secret_hash FROM oauth_client WHERE client_id='governance-reset-prod'",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "client_secret_hash")
        .unwrap();
    assert!(bcrypt::verify("fixture-client-secret", &production_hash).unwrap());
    let repeat = service
        .mutate(
            &owner,
            a.app,
            change,
            "reset-once",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(repeat.newly_issued_secret.is_none());
    assert_eq!(repeat.receipt.operation_id, first.receipt.operation_id);
    assert_eq!(scalar(&f,"SELECT secret_revision AS value FROM oauth_client WHERE client_id='governance-reset-dev'").await, 1);
    assert_eq!(
        scalar(
            &f,
            &format!(
                "SELECT revision AS value FROM identity_app_environment WHERE id='{}'",
                a.production
            )
        )
        .await,
        0
    );
    f.db.execute_unprepared("CREATE FUNCTION reject_governance_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='application.secret.reset' THEN RAISE EXCEPTION 'fixture rejected commit'; END IF; RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER reject_governance_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_governance_commit()").await.unwrap();
    assert!(
        service
            .mutate(
                &owner,
                a.app,
                ApplicationMutation::ResetSecret {
                    environment_id: a.development,
                    expected_revision: 1,
                },
                "failed-reset",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .is_err()
    );
    assert_eq!(
        scalar(
            &f,
            &format!(
                "SELECT revision AS value FROM identity_app_environment WHERE id='{}'",
                a.development
            )
        )
        .await,
        1
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_request_operation WHERE idempotency_key='failed-reset'").await,0);
    assert_eq!(scalar(&f,"SELECT secret_revision AS value FROM oauth_client WHERE client_id='governance-reset-dev'").await, 1);
    let current_hash: String =
        f.db.query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT client_secret_hash FROM oauth_client WHERE client_id='governance-reset-dev'",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "client_secret_hash")
        .unwrap();
    assert_eq!(current_hash, hash);
    f.close().await;
}

#[tokio::test]
async fn identity_application_governance_candidates_owner_transfer_and_overlapping_member_sources_are_isolated()
 {
    let f = IdentityFixture::start().await;
    let a = app(&f, "governance-members", true).await;
    developer_managed(&f, &a).await;
    let target = second_developer(&f, false).await;
    let (_, owner_secret) = center_secret(&f, a.owner).await;
    let owner = write_evidence(
        &f,
        Some(&owner_secret),
        &IssuedSecret::from_entropy([9; 32]).into_response(),
    );
    let service = service(&f);
    let candidates = service
        .candidates(&owner_secret, 0, 50, None, &budget(&f))
        .await
        .unwrap();
    assert!(candidates.items.iter().all(|c| c.user_id != target));
    let add = ApplicationMutation::SetMember {
        user_id: target,
        present: true,
        expected_revision: 0,
    };
    assert_eq!(
        service
            .mutate(
                &owner,
                a.app,
                add.clone(),
                "unseen-member",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::Forbidden)
    );
    service
        .set_candidate(&owner, a.owner, target, true, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    let candidates = service
        .candidates(&owner_secret, 0, 50, None, &budget(&f))
        .await
        .unwrap();
    assert!(candidates.items.iter().any(|c| c.user_id == target));
    let searched = service
        .candidates(&owner_secret, 0, 50, Some(&target.to_string()), &budget(&f))
        .await
        .unwrap();
    assert_eq!(searched.items.len(), 1);
    assert_eq!(searched.items[0].user_id, target);
    let missing = service
        .candidates(&owner_secret, 0, 50, Some("no-such-candidate"), &budget(&f))
        .await
        .unwrap();
    assert!(missing.items.is_empty());
    service
        .mutate(
            &owner,
            a.app,
            add,
            "add-member",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    let target_root = center(&f, target).await;
    let (_, _, rp) = identity_tokens(&f, &a, &target_root, "openid").await;
    let member = evidence(&f, target).await;
    assert_eq!(
        service
            .mutate(
                &member,
                a.app,
                ApplicationMutation::TransferOwner {
                    user_id: target,
                    expected_revision: 1
                },
                "maintainer-owner",
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::Forbidden)
    );
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::SetTestUser {
        environment_id: a.development,
        user_id: target,
        present: true,
        actor_id: a.owner,
    })
    .await
    .unwrap();
    tx.commit().await.unwrap();
    service
        .mutate(
            &owner,
            a.app,
            ApplicationMutation::SetMember {
                user_id: target,
                present: false,
                expected_revision: 2,
            },
            "remove-member",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE id='{rp}' AND revoked_at IS NULL")).await,1);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery"
        )
        .await,
        0
    );
    let transfer = ApplicationMutation::TransferOwner {
        user_id: target,
        expected_revision: 3,
    };
    let first_budget = budget(&f);
    let second_budget = budget(&f);
    let (one, two) = tokio::join!(
        service.mutate(
            &owner,
            a.app,
            transfer.clone(),
            "transfer-one",
            Uuid::new_v4(),
            &first_budget
        ),
        service.mutate(
            &owner,
            a.app,
            transfer,
            "transfer-two",
            Uuid::new_v4(),
            &second_budget
        )
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_app_member WHERE member_role='owner'"
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
    f.close().await;
}
