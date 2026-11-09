use super::{super::common::IdentityFixture, identity_support::*};
use auth::security::identity::SecretToken;
use cobotusr::{
    domain::identity::{
        account::{IssuedSecret, PersonKind},
        application::IdentityScopeSet,
        error::IdentityError,
    },
    service::identity::{
        assignment::{AssignmentMutation, AssignmentService},
        repository::*,
    },
};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use std::sync::Arc;
use uuid::Uuid;

fn service(f: &IdentityFixture) -> AssignmentService {
    AssignmentService::new(Arc::new(repository(f)), f.clock.clone())
}
async fn evidence(
    f: &IdentityFixture,
    user: i32,
) -> cobotusr::service::identity::browser::BrowserWriteEvidence {
    let (_, secret) = center_secret(f, user).await;
    write_evidence(
        f,
        Some(&secret),
        &IssuedSecret::from_entropy([19; 32]).into_response(),
    )
}
async fn collaborator(f: &IdentityFixture) -> i32 {
    let mut tx = repository(f).begin(&budget(f)).await.unwrap();
    let IdentityWriteResult::Account(account) = tx
        .apply(IdentityWrite::CreateAccount(NewAccountWrite {
            username: "assignment-collaborator".into(),
            nickname: "Partner".into(),
            password_hash: SecretToken::new("fixture-unused".into()).unwrap(),
            person_kind: PersonKind::Collaborator,
            developer_enabled: false,
        }))
        .await
        .unwrap()
    else {
        panic!("account")
    };
    tx.commit().await.unwrap();
    account.user_id
}
fn group_id(r: &SafeOperationReceipt) -> Uuid {
    serde_json::from_value(r.summary["group_id"].clone()).unwrap()
}

#[tokio::test]
async fn identity_assignment_service_group_sources_revoke_only_final_loss_and_never_implicitly_assign()
 {
    let f = IdentityFixture::start().await;
    let a = app(&f, "service-assignment", true).await;
    let other = app(&f, "service-unassigned", false).await;
    let user = collaborator(&f).await;
    let e = evidence(&f, a.owner).await;
    let s = service(&f);
    let create = AssignmentMutation::CreateGroup {
        name: " Staff ".into(),
        description: None,
    };
    let first = s
        .mutate(
            &e,
            create.clone(),
            "group-create",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    let group = group_id(&first);
    let repeat = s
        .mutate(&e, create, "group-create", Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    assert_eq!(first.operation_id, repeat.operation_id);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_group_member"
        )
        .await,
        0
    );
    s.mutate(
        &e,
        AssignmentMutation::SetGroupMember {
            group_id: group,
            user_id: user,
            present: true,
            expected_revision: 0,
        },
        "group-add",
        Uuid::new_v4(),
        &budget(&f),
    )
    .await
    .unwrap();
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_app_assignment"
        )
        .await,
        0
    );
    s.mutate(
        &e,
        AssignmentMutation::SetAssignment {
            application_id: a.app,
            target: AssignmentTarget::Group(group),
            present: true,
            expected_revision: 0,
        },
        "assign-group",
        Uuid::new_v4(),
        &budget(&f),
    )
    .await
    .unwrap();
    s.mutate(
        &e,
        AssignmentMutation::SetAssignment {
            application_id: a.app,
            target: AssignmentTarget::User(user),
            present: true,
            expected_revision: 1,
        },
        "assign-user",
        Uuid::new_v4(),
        &budget(&f),
    )
    .await
    .unwrap();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
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
    tx.commit().await.unwrap();
    let root = center(&f, user).await;
    let (_, _, rp) = identity_tokens_for_environment(&f, &a, &root, "openid", a.production).await;
    s.mutate(
        &e,
        AssignmentMutation::SetAssignment {
            application_id: a.app,
            target: AssignmentTarget::User(user),
            present: false,
            expected_revision: 2,
        },
        "unassign-user",
        Uuid::new_v4(),
        &budget(&f),
    )
    .await
    .unwrap();
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE id='{rp}' AND revoked_at IS NULL")).await,1);
    let removal = s
        .mutate(
            &e,
            AssignmentMutation::SetGroupMember {
                group_id: group,
                user_id: user,
                present: false,
                expected_revision: 1,
            },
            "group-remove",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(removal.summary["affected_count"], 1);
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery WHERE rp_session_id='{rp}'")).await,1);
    assert_eq!(scalar(&f,&format!("SELECT generation AS value FROM identity_access_generation WHERE environment_id='{}' AND user_id={user}",a.development)).await,0);
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_app_assignment WHERE application_id={}",other.app)).await,0);
    s.mutate(
        &e,
        AssignmentMutation::SetGroupMember {
            group_id: group,
            user_id: user,
            present: true,
            expected_revision: 2,
        },
        "group-readd",
        Uuid::new_v4(),
        &budget(&f),
    )
    .await
    .unwrap();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let admission = tx
        .evaluate_admission(&AdmissionQuery {
            sso_session_id: root.id,
            environment_id: a.production,
            requested_scopes: IdentityScopeSet::parse("openid").unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(admission.access_generation(), 1);
    tx.commit().await.unwrap();
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE id='{rp}' AND revoked_at IS NULL")).await,0);
    s.mutate(
        &e,
        AssignmentMutation::DeleteGroup {
            group_id: group,
            expected_revision: 3,
        },
        "delete-group",
        Uuid::new_v4(),
        &budget(&f),
    )
    .await
    .unwrap();
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_user_group"
        )
        .await,
        0
    );
    assert_eq!(scalar(&f,&format!("SELECT generation AS value FROM identity_access_generation WHERE environment_id='{}' AND user_id={user}",a.production)).await,2);
    f.close().await;
}

#[tokio::test]
async fn identity_assignment_service_authority_revision_and_commit_failure_are_atomic() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "service-failure", false).await;
    let user = collaborator(&f).await;
    let e = evidence(&f, a.owner).await;
    let outsider = evidence(&f, user).await;
    let s = service(&f);
    let create = AssignmentMutation::CreateGroup {
        name: "Atomic group".into(),
        description: None,
    };
    assert_eq!(
        s.mutate(
            &outsider,
            create.clone(),
            "forbidden-create",
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .err(),
        Some(IdentityError::Forbidden)
    );
    let group = group_id(
        &s.mutate(&e, create, "atomic-create", Uuid::new_v4(), &budget(&f))
            .await
            .unwrap(),
    );
    let add = AssignmentMutation::SetGroupMember {
        group_id: group,
        user_id: user,
        present: true,
        expected_revision: 1,
    };
    assert_eq!(
        s.mutate(&e, add, "stale-member", Uuid::new_v4(), &budget(&f))
            .await
            .err(),
        Some(IdentityError::Conflict)
    );
    f.db.execute_unprepared("CREATE FUNCTION reject_assignment_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='group.member.change' THEN RAISE EXCEPTION 'fixture commit failure'; END IF; RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER reject_assignment_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_assignment_commit()").await.unwrap();
    let add = AssignmentMutation::SetGroupMember {
        group_id: group,
        user_id: user,
        present: true,
        expected_revision: 0,
    };
    assert!(
        s.mutate(&e, add, "failed-add", Uuid::new_v4(), &budget(&f))
            .await
            .is_err()
    );
    assert_eq!(
        scalar(
            &f,
            &format!("SELECT revision AS value FROM identity_user_group WHERE id='{group}'")
        )
        .await,
        0
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_group_member"
        )
        .await,
        0
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_request_operation WHERE idempotency_key='failed-add'").await,0);
    f.db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) SELECT $1,id,FALSE,NOW(),NOW() FROM authority WHERE method='POST' AND authority_uri='/admin/groups'",[a.owner.into()])).await.unwrap();
    assert_eq!(
        s.mutate(
            &e,
            AssignmentMutation::CreateGroup {
                name: "Denied group".into(),
                description: None
            },
            "denied-create",
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .err(),
        Some(IdentityError::Forbidden)
    );
    f.close().await;
}
