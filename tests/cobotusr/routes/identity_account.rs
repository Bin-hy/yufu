//! Account onboarding, qualification and action-flow integration scenarios.
use super::super::common::IdentityFixture;
use super::identity_support::*;
use cobotusr::domain::identity::{
    account::{InvitationPurpose, PersonKind, SecretDigest},
    error::IdentityError,
};
use cobotusr::service::identity::repository::*;
use uuid::Uuid;

#[tokio::test]
async fn identity_account_service_qualification_change_preserves_other_sources_and_disable_revokes_roots()
 {
    use auth::security::identity::SecretToken;
    use cobotusr::service::identity::account::AccountService;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    use std::sync::Arc;
    let f = IdentityFixture::start().await;
    let a = app(&f, "qualification-service", true).await;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE application SET management_mode='developer_managed' WHERE id=$1",
        [a.app.into()],
    ))
    .await
    .unwrap();
    let (_, actor_secret) = center_secret(&f, a.owner).await;
    let browser =
        cobotusr::domain::identity::account::IssuedSecret::from_entropy([8; 32]).into_response();
    let evidence = write_evidence(&f, Some(&actor_secret), &browser);
    let service =
        AccountService::new(Arc::new(repository(&f)), f.clock.clone(), f.config.clone()).unwrap();
    let revision = scalar(
        &f,
        &format!(
            "SELECT identity_revision AS value FROM \"user\" WHERE id={}",
            a.owner
        ),
    )
    .await as u64;
    assert_eq!(
        service
            .set_developer(
                &evidence,
                a.owner,
                false,
                revision,
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::OwnerRequired)
    );
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let IdentityWriteResult::Account(target) = tx
        .apply(IdentityWrite::CreateAccount(NewAccountWrite {
            username: "qualification-member".into(),
            password_hash: SecretToken::new("unused-fixture-hash".into()).unwrap(),
            nickname: "Member".into(),
            person_kind: PersonKind::Employee,
            developer_enabled: true,
        }))
        .await
        .unwrap()
    else {
        panic!("account expected")
    };
    let app_revision = scalar(
        &f,
        &format!(
            "SELECT identity_revision AS value FROM application WHERE id={}",
            a.app
        ),
    )
    .await;
    tx.apply(IdentityWrite::SetMember {
        application_id: a.app,
        user_id: target.user_id,
        present: true,
        actor_id: a.owner,
        expected_revision: app_revision as u64,
    })
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let (root, secret) = center_secret(&f, target.user_id).await;
    let (_, _, first_rp) = identity_tokens(&f, &a, &root, "openid").await;
    let result = service
        .set_developer(
            &evidence,
            target.user_id,
            false,
            0,
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(result.auth_epoch, 0);
    assert_eq!(result.revoked_rp_count, 1);
    assert_eq!(scalar(&f, &format!("SELECT generation AS value FROM identity_access_generation WHERE environment_id='{}' AND user_id={}",a.development,target.user_id)).await, 1);
    assert_eq!(scalar(&f, &format!("SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE id='{first_rp}' AND revoked_at IS NOT NULL")).await, 1);
    service
        .set_developer(
            &evidence,
            target.user_id,
            true,
            1,
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    let (_, _, second_rp) = identity_tokens(&f, &a, &root, "openid").await;
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::SetTestUser {
        environment_id: a.development,
        user_id: target.user_id,
        present: true,
        actor_id: a.owner,
    })
    .await
    .unwrap();
    tx.apply(IdentityWrite::RecomputeGeneration {
        environment_id: a.development,
        user_id: target.user_id,
    })
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let result = service
        .set_developer(
            &evidence,
            target.user_id,
            false,
            2,
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(result.revoked_rp_count, 0);
    assert_eq!(scalar(&f, &format!("SELECT generation AS value FROM identity_access_generation WHERE environment_id='{}' AND user_id={}",a.development,target.user_id)).await, 1);
    assert_eq!(scalar(&f, &format!("SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE id='{second_rp}' AND revoked_at IS NULL")).await, 1);
    let before = scalar(
        &f,
        "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery",
    )
    .await;
    assert_eq!(
        service
            .set_enabled(
                &evidence,
                target.user_id,
                false,
                2,
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::Conflict)
    );
    let result = service
        .set_enabled(
            &evidence,
            target.user_id,
            false,
            3,
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(result.auth_epoch, 1);
    assert_eq!(result.revoked_rp_count, 1);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery"
        )
        .await,
        before + 1
    );
    let result = service
        .set_enabled(
            &evidence,
            target.user_id,
            true,
            4,
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(result.auth_epoch, 1);
    let sso = cobotusr::service::identity::sso::SsoService::new(
        Arc::new(repository(&f)),
        f.clock.clone(),
        f.config.clone(),
    )
    .unwrap();
    assert!(
        sso.authenticate_browser(&secret, &budget(&f))
            .await
            .is_err()
    );
    assert_eq!(scalar(&f,&format!("SELECT COUNT(*)::bigint AS value FROM identity_subject WHERE sub='{}' AND user_id={} AND tombstoned_at IS NULL",target.user_id,target.user_id)).await,1);
    // Faulting the final audit must undo both the account write and revocations.
    f.db.execute_unprepared("CREATE FUNCTION fail_qualification_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='account.developer.change' THEN RAISE EXCEPTION 'fixture commit rejected'; END IF; RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER fail_qualification_commit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_qualification_commit()").await.unwrap();
    assert!(
        service
            .set_developer(
                &evidence,
                target.user_id,
                true,
                5,
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .is_err()
    );
    let row =
        f.db.query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT identity_revision,developer_enabled FROM \"user\" WHERE id=$1",
            [target.user_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "identity_revision").unwrap(), 5);
    assert!(!row.try_get::<bool>("", "developer_enabled").unwrap());
    f.close().await;
}

#[tokio::test]
async fn identity_account_service_concurrent_admin_disable_preserves_last_administrator() {
    use auth::security::identity::SecretToken;
    use cobotusr::service::identity::account::AccountService;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    use std::sync::Arc;
    let f = IdentityFixture::start().await;
    let first = admin_id(&f).await;
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let IdentityWriteResult::Account(second) = tx
        .apply(IdentityWrite::CreateAccount(NewAccountWrite {
            username: "second-admin".into(),
            password_hash: SecretToken::new("fixture-hash".into()).unwrap(),
            nickname: "Second".into(),
            person_kind: PersonKind::Employee,
            developer_enabled: false,
        }))
        .await
        .unwrap()
    else {
        panic!("account expected")
    };
    tx.commit().await.unwrap();
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET role_id=(SELECT role_id FROM \"user\" WHERE id=$1) WHERE id=$2",
        [first.into(), second.user_id.into()],
    ))
    .await
    .unwrap();
    let (_, a) = center_secret(&f, first).await;
    let (_, b) = center_secret(&f, second.user_id).await;
    let browser =
        cobotusr::domain::identity::account::IssuedSecret::from_entropy([9; 32]).into_response();
    let one = write_evidence(&f, Some(&a), &browser);
    let two = write_evidence(&f, Some(&b), &browser);
    let service =
        AccountService::new(Arc::new(repository(&f)), f.clock.clone(), f.config.clone()).unwrap();
    let first_budget = budget(&f);
    let second_budget = budget(&f);
    let (one, two) = tokio::join!(
        service.set_enabled(&one, first, false, 0, Uuid::new_v4(), &first_budget),
        service.set_enabled(
            &two,
            second.user_id,
            false,
            0,
            Uuid::new_v4(),
            &second_budget
        )
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert_eq!(
        one.err().or(two.err()),
        Some(IdentityError::LastAdministrator)
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM \"user\" WHERE enabled"
        )
        .await,
        1
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='account.enabled.change'").await,1);
    f.close().await;
}

#[tokio::test]
async fn identity_account_service_password_reset_and_change_revoke_all_identity_roots_atomically() {
    use auth::security::identity::SecretToken;
    use cobotusr::{
        domain::identity::account::{AccountPassword, IssuedSecret},
        service::{
            identity::{
                account::{AccountService, ActionProof},
                sso::SsoService,
            },
            security::{lifecycle::CredentialLifecycle, repository::PgSecurityRepository},
        },
    };
    use sea_orm::ConnectionTrait;
    use std::sync::Arc;
    let f = IdentityFixture::start().await;
    let a = app(&f, "password-web", true).await;
    let (first_root, first_secret) = center_secret(&f, a.owner).await;
    let (second_root, _) = center_secret(&f, a.owner).await;
    let (first_tokens, _, _) = identity_tokens(&f, &a, &first_root, "openid").await;
    let (second_tokens, _, _) = identity_tokens(&f, &a, &second_root, "openid").await;
    let browser = IssuedSecret::from_entropy([11; 32]).into_response();
    let admin = write_evidence(&f, Some(&first_secret), &browser);
    let anonymous = write_evidence(&f, None, &browser);
    let service =
        AccountService::new(Arc::new(repository(&f)), f.clock.clone(), f.config.clone()).unwrap();
    let sso = SsoService::new(Arc::new(repository(&f)), f.clock.clone(), f.config.clone()).unwrap();
    let old = AccountPassword::new("fixture-admin-password".into()).unwrap();
    let wrong = AccountPassword::new("wrong password".into()).unwrap();
    let new = AccountPassword::new("new reset password with spaces".into()).unwrap();
    assert_eq!(
        service
            .change_password(&admin, &wrong, &new, Uuid::new_v4(), &budget(&f))
            .await
            .err(),
        Some(IdentityError::Unauthenticated)
    );
    assert_eq!(scalar(&f, "SELECT COUNT(*)::bigint AS value FROM identity_sso_session WHERE revoked_at IS NOT NULL").await, 0);
    let reset = service
        .issue_reset(&admin, a.owner, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    let stale = service
        .issue_reset(&admin, a.owner, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    let reset_secret = SecretToken::new(reset.secret.into_response()).unwrap();
    let flow = service
        .exchange_action(
            "password_reset",
            &reset_secret,
            &anonymous,
            None,
            &budget(&f),
        )
        .await
        .unwrap();
    let proof = SecretToken::new(flow.proof.into_response()).unwrap();
    // A deferred audit rejection occurs after the credential and root writes.
    f.db.execute_unprepared("CREATE FUNCTION reject_password_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='account.password.reset' THEN RAISE EXCEPTION 'test audit commit rejection'; END IF; RETURN NEW; END $$").await.unwrap();
    f.db.execute_unprepared("CREATE CONSTRAINT TRIGGER reject_password_audit AFTER INSERT ON identity_audit_event DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_password_audit()").await.unwrap();
    assert!(
        service
            .reset_password(
                &anonymous,
                ActionProof {
                    action_id: flow.action_id,
                    proof: &proof
                },
                &new,
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .is_err()
    );
    assert_eq!(scalar(&f, "SELECT COUNT(*)::bigint AS value FROM identity_sso_session WHERE revoked_at IS NOT NULL").await, 0);
    assert_eq!(scalar(&f, "SELECT COUNT(*)::bigint AS value FROM identity_password_reset WHERE consumed_at IS NOT NULL").await, 0);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery"
        )
        .await,
        0
    );
    f.db.execute_unprepared("DROP TRIGGER reject_password_audit ON identity_audit_event")
        .await
        .unwrap();
    let before = scalar(
        &f,
        &format!(
            "SELECT auth_epoch AS value FROM \"user\" WHERE id={}",
            a.owner
        ),
    )
    .await;
    let complete = || async {
        service
            .reset_password(
                &anonymous,
                ActionProof {
                    action_id: flow.action_id,
                    proof: &proof,
                },
                &new,
                Uuid::new_v4(),
                &budget(&f),
            )
            .await
    };
    let (one, two) = tokio::join!(complete(), complete());
    assert_eq!([one, two].into_iter().filter(Result::is_ok).count(), 1);
    assert_eq!(
        scalar(
            &f,
            &format!(
                "SELECT auth_epoch AS value FROM \"user\" WHERE id={}",
                a.owner
            )
        )
        .await,
        before + 1
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery"
        )
        .await,
        2
    );
    let lifecycle = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(f.db.clone(), f.clock.clone())),
        f.clock.clone(),
    );
    for tokens in [&first_tokens, &second_tokens] {
        assert!(
            lifecycle
                .check_registered(
                    &auth::security::credential::CredentialRef::new(
                        &f.base_url,
                        &tokens.claims.jti
                    )
                    .unwrap(),
                    &budget(&f)
                )
                .await
                .is_err()
        );
    }
    assert_eq!(
        service
            .exchange_action(
                "password_reset",
                &SecretToken::new(stale.secret.into_response()).unwrap(),
                &anonymous,
                None,
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::AccessDenied)
    );
    assert_eq!(
        sso.authenticate_password(
            "identity-admin",
            old.expose_secret(),
            &anonymous,
            None,
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .err()
        .map(|e| e.code()),
        Some("unauthenticated")
    );
    let login = sso
        .authenticate_password(
            "identity-admin",
            new.expose_secret(),
            &anonymous,
            None,
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    let new_secret = login.cookie_secret.into_response();
    let authenticated = write_evidence(&f, Some(&new_secret), &browser);
    let changed = AccountPassword::new("changed personal password".into()).unwrap();
    service
        .change_password(&authenticated, &new, &changed, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    assert!(
        sso.authenticate_browser(&new_secret, &budget(&f))
            .await
            .is_err()
    );
    assert_eq!(
        scalar(
            &f,
            &format!(
                "SELECT auth_epoch AS value FROM \"user\" WHERE id={}",
                a.owner
            )
        )
        .await,
        before + 2
    );
    assert!(
        sso.authenticate_password(
            "identity-admin",
            changed.expose_secret(),
            &anonymous,
            None,
            Uuid::new_v4(),
            &budget(&f)
        )
        .await
        .is_ok()
    );
    f.close().await;
}

#[tokio::test]
async fn identity_account_service_profile_changes_keep_subject_and_clear_email_verification() {
    use cobotusr::{
        domain::identity::account::IssuedSecret, service::identity::account::AccountService,
    };
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    use std::sync::Arc;
    let f = IdentityFixture::start().await;
    let administrator = admin_id(&f).await;
    let (root, center) = center_secret(&f, administrator).await;
    let browser = IssuedSecret::from_entropy([12; 32]).into_response();
    let evidence = write_evidence(&f, Some(&center), &browser);
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET email='verified@example.test',email_verified_at=1 WHERE id=$1",
        [administrator.into()],
    ))
    .await
    .unwrap();
    let service =
        AccountService::new(Arc::new(repository(&f)), f.clock.clone(), f.config.clone()).unwrap();
    let unchanged = service
        .update_profile(
            &evidence,
            "New Name",
            Some("verified@example.test"),
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(unchanged.email_verified, Some(true));
    let changed = service
        .update_profile(
            &evidence,
            "New Name",
            Some("new@example.test"),
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(changed.email_verified, Some(false));
    assert_eq!(
        changed.preferred_username.as_deref(),
        Some("identity-admin")
    );
    let missing = service
        .update_profile(&evidence, "New Name", None, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    assert!(missing.email.is_none() && missing.email_verified.is_none());
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let restored = tx
        .load_browser_session(&SecretDigest::of(&center))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.principal, root.principal);
    assert_eq!(restored.authenticated_at, root.authenticated_at);
    tx.rollback().await.unwrap();
    f.close().await;
}

#[tokio::test]
async fn identity_account_service_issues_admin_only_actions_with_fixed_ttl_and_epoch() {
    use auth::security::{budget::Clock, identity::SecretToken};
    use cobotusr::{
        domain::identity::account::IssuedSecret,
        service::identity::account::{AccountService, InvitationRequest},
    };
    use std::sync::Arc;
    let f = IdentityFixture::start().await;
    let administrator = admin_id(&f).await;
    let (_, center) = center_secret(&f, administrator).await;
    let browser = IssuedSecret::from_entropy([3; 32]).into_response();
    let admin = write_evidence(&f, Some(&center), &browser);
    let anonymous = write_evidence(&f, None, &browser);
    let service =
        AccountService::new(Arc::new(repository(&f)), f.clock.clone(), f.config.clone()).unwrap();
    let request = InvitationRequest {
        purpose: InvitationPurpose::Onboarding,
        target_user_id: None,
        person_kind: PersonKind::Collaborator,
        developer_grant: false,
    };
    assert_eq!(
        service
            .issue_invitation(&anonymous, request, Uuid::new_v4(), &budget(&f))
            .await
            .err(),
        Some(IdentityError::Unauthenticated)
    );
    let invitation = service
        .issue_invitation(&admin, request, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    assert_eq!(
        invitation.expires_at,
        f.clock.now().unwrap().unix_millis + 86_400_000
    );
    let secret = SecretToken::new(invitation.secret.into_response()).unwrap();
    let exchanged = service
        .exchange_action("invitation", &secret, &anonymous, None, &budget(&f))
        .await
        .unwrap();
    assert_eq!(
        exchanged.expires_at,
        f.clock.now().unwrap().unix_millis + 600_000
    );
    assert!(exchanged.oidc_flow_id.is_none());
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let source = tx
        .invitation_by_digest(&SecretDigest::of(secret.expose_secret()))
        .await
        .unwrap()
        .unwrap();
    assert!(source.consumed_at.is_none());
    let flow = tx
        .load_action_flow(exchanged.action_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(flow.request_payload, serde_json::json!({}));
    assert_eq!(flow.browser_binding_digest, SecretDigest::of(&browser));
    tx.rollback().await.unwrap();
    service
        .revoke_action(&admin, flow.target, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    assert_eq!(
        service
            .exchange_action("invitation", &secret, &anonymous, None, &budget(&f))
            .await
            .err(),
        Some(IdentityError::AccessDenied)
    );
    let reset = service
        .issue_reset(&admin, administrator, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    assert_eq!(
        reset.expires_at,
        f.clock.now().unwrap().unix_millis + 1_800_000
    );
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let persisted = tx.load_reset(reset.id).await.unwrap().unwrap();
    assert_eq!(
        persisted.auth_epoch_at_issue,
        tx.load_account(administrator)
            .await
            .unwrap()
            .unwrap()
            .auth_epoch
    );
    tx.rollback().await.unwrap();
    f.clock.advance(1_800_000);
    assert_eq!(
        service
            .exchange_action(
                "password_reset",
                &SecretToken::new(reset.secret.into_response()).unwrap(),
                &anonymous,
                None,
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::Expired)
    );
    f.close().await;
}

#[tokio::test]
async fn identity_account_service_invitation_onboarding_is_atomic_and_does_not_grant_business_access()
 {
    use auth::security::identity::SecretToken;
    use cobotusr::{
        domain::identity::account::IssuedSecret,
        service::identity::account::{
            AccountService, ActionProof, InvitationAcceptance, InvitationRequest,
        },
    };
    use std::sync::Arc;
    let f = IdentityFixture::start().await;
    let administrator = admin_id(&f).await;
    let (_, center) = center_secret(&f, administrator).await;
    let browser = IssuedSecret::from_entropy([5; 32]).into_response();
    let admin = write_evidence(&f, Some(&center), &browser);
    let anonymous = write_evidence(&f, None, &browser);
    let service =
        AccountService::new(Arc::new(repository(&f)), f.clock.clone(), f.config.clone()).unwrap();
    let request = InvitationRequest {
        purpose: InvitationPurpose::Onboarding,
        target_user_id: None,
        person_kind: PersonKind::Collaborator,
        developer_grant: true,
    };
    let invitation = service
        .issue_invitation(&admin, request, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    let secret = SecretToken::new(invitation.secret.into_response()).unwrap();
    let flow = service
        .exchange_action("invitation", &secret, &anonymous, None, &budget(&f))
        .await
        .unwrap();
    let proof = SecretToken::new(flow.proof.into_response()).unwrap();
    let password = cobotusr::domain::identity::account::AccountPassword::new(
        "valid invitation password".into(),
    )
    .unwrap();
    let wrong_browser = write_evidence(
        &f,
        None,
        &IssuedSecret::from_entropy([6; 32]).into_response(),
    );
    assert_eq!(
        service
            .accept_invitation(
                &wrong_browser,
                ActionProof {
                    action_id: flow.action_id,
                    proof: &proof
                },
                InvitationAcceptance::NewAccount {
                    username: "invited-collaborator",
                    password: &password,
                    nickname: "Partner"
                },
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::Forbidden)
    );
    let result = service
        .accept_invitation(
            &anonymous,
            ActionProof {
                action_id: flow.action_id,
                proof: &proof,
            },
            InvitationAcceptance::NewAccount {
                username: "invited-collaborator",
                password: &password,
                nickname: "Partner",
            },
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(result.requires_login);
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let account = tx.load_account(result.user_id).await.unwrap().unwrap();
    assert_eq!(account.person_kind, PersonKind::Collaborator);
    assert!(account.developer_enabled && !account.administrator);
    tx.rollback().await.unwrap();
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_app_assignment"
        )
        .await,
        0
    );
    assert_eq!(
        scalar(&f, "SELECT COUNT(*)::bigint AS value FROM user_permission").await,
        0
    );
    assert_eq!(
        service
            .accept_invitation(
                &anonymous,
                ActionProof {
                    action_id: flow.action_id,
                    proof: &proof
                },
                InvitationAcceptance::ExistingAccount,
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::AlreadyConsumed)
    );
    let (_, partner_center) = center_secret(&f, result.user_id).await;
    let partner = write_evidence(&f, Some(&partner_center), &browser);
    assert_eq!(
        service
            .issue_invitation(&partner, request, Uuid::new_v4(), &budget(&f))
            .await
            .err(),
        Some(IdentityError::Forbidden)
    );
    let collision = service
        .issue_invitation(&admin, request, Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    let collision_secret = SecretToken::new(collision.secret.into_response()).unwrap();
    let collision_flow = service
        .exchange_action(
            "invitation",
            &collision_secret,
            &anonymous,
            None,
            &budget(&f),
        )
        .await
        .unwrap();
    let collision_proof = SecretToken::new(collision_flow.proof.into_response()).unwrap();
    assert_eq!(
        service
            .accept_invitation(
                &anonymous,
                ActionProof {
                    action_id: collision_flow.action_id,
                    proof: &collision_proof
                },
                InvitationAcceptance::NewAccount {
                    username: "invited-collaborator",
                    password: &password,
                    nickname: "Duplicate"
                },
                Uuid::new_v4(),
                &budget(&f)
            )
            .await
            .err(),
        Some(IdentityError::Conflict)
    );
    let linked = service
        .accept_invitation(
            &partner,
            ActionProof {
                action_id: collision_flow.action_id,
                proof: &collision_proof,
            },
            InvitationAcceptance::ExistingAccount,
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert_eq!(linked.user_id, result.user_id);
    assert!(!linked.requires_login);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM \"user\" WHERE username='invited-collaborator'"
        )
        .await,
        1
    );
    f.close().await;
}

#[tokio::test]
async fn identity_account_action_flow_rejects_wrong_browser_purpose_and_parallel_reuse() {
    use auth::security::budget::Clock;
    use cobotusr::domain::identity::session::{ActionFlowSnapshot, ActionTarget};
    let f = IdentityFixture::start().await;
    let actor = admin_id(&f).await;
    let invitation = Uuid::new_v4();
    let action = Uuid::new_v4();
    let target = ActionTarget::Invitation(invitation);
    let browser = SecretDigest::of("anonymous-cookie");
    let proof = SecretDigest::of("action-specific-proof");
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::IssueInvitation(InvitationWrite {
        id: invitation,
        digest: SecretDigest::of("original-invitation"),
        purpose: InvitationPurpose::Qualification,
        target_user_id: Some(actor),
        person_kind: PersonKind::Employee,
        developer_grant: true,
        created_by: actor,
        expires_at: f.clock.now().unwrap().unix_millis + 60_000,
    }))
    .await
    .unwrap();
    tx.apply(IdentityWrite::CreateActionFlow(ActionFlowSnapshot {
        id: action,
        target,
        browser_binding_digest: browser.clone(),
        browser_proof_digest: proof.clone(),
        expires_at: f.clock.now().unwrap().unix_millis + 30_000,
        consumed_at: None,
        request_payload: serde_json::json!({}),
    }))
    .await
    .unwrap();
    tx.commit().await.unwrap();
    for (wrong_target, wrong_browser, wrong_proof) in [
        (
            ActionTarget::PasswordReset(invitation),
            browser.clone(),
            proof.clone(),
        ),
        (target, SecretDigest::of("other-cookie"), proof.clone()),
        (target, browser.clone(), SecretDigest::of("other-action")),
    ] {
        let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
        assert_eq!(
            tx.apply(IdentityWrite::ConsumeActionFlow {
                id: action,
                target: wrong_target,
                browser_binding_digest: wrong_browser,
                browser_proof_digest: wrong_proof
            })
            .await
            .err(),
            Some(IdentityError::Forbidden)
        );
        tx.rollback().await.unwrap();
    }
    let barrier = tokio::sync::Barrier::new(2);
    let consume = || async {
        let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
        barrier.wait().await;
        match tx
            .apply(IdentityWrite::ConsumeActionFlow {
                id: action,
                target,
                browser_binding_digest: browser.clone(),
                browser_proof_digest: proof.clone(),
            })
            .await
        {
            Ok(_) => tx.commit().await.map_err(IdentityError::from),
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    };
    let (one, two) = tokio::join!(consume(), consume());
    assert_eq!([one, two].into_iter().filter(Result::is_ok).count(), 1);
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    assert!(
        tx.load_action_flow(action)
            .await
            .unwrap()
            .unwrap()
            .consumed_at
            .is_some()
    );
    let source = tx
        .invitation_by_digest(&SecretDigest::of("original-invitation"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(source.target_user_id, Some(actor));
    assert!(source.consumed_at.is_none());
    tx.commit().await.unwrap();
    f.close().await;
}

#[tokio::test]
async fn identity_account_profile_returns_only_real_optional_email() {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let f = IdentityFixture::start().await;
    let actor = admin_id(&f).await;
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let profile = tx.load_profile(actor).await.unwrap().unwrap();
    assert!(profile.email.is_none());
    assert!(profile.email_verified.is_none());
    tx.commit().await.unwrap();
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET email=$2,email_verified_at=$3 WHERE id=$1",
        [actor.into(), "actual@example.test".into(), 1_i64.into()],
    ))
    .await
    .unwrap();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let profile = tx.load_profile(actor).await.unwrap().unwrap();
    assert_eq!(profile.email.as_deref(), Some("actual@example.test"));
    assert_eq!(profile.email_verified, Some(true));
    tx.commit().await.unwrap();
    f.close().await;
}

#[tokio::test]
async fn identity_account_concurrent_invitation_consumption_has_one_winner() {
    let f = IdentityFixture::start().await;
    let owner = admin_id(&f).await;
    let invitation = Uuid::new_v4();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::IssueInvitation(InvitationWrite {
        id: invitation,
        digest: SecretDigest::of("one-use-invitation"),
        purpose: InvitationPurpose::Qualification,
        target_user_id: Some(owner),
        person_kind: PersonKind::Employee,
        developer_grant: true,
        created_by: owner,
        expires_at: auth::security::budget::Clock::now(f.clock.as_ref())
            .unwrap()
            .unix_millis
            + 60_000,
    }))
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let barrier = tokio::sync::Barrier::new(2);
    let consume = || async {
        let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
        barrier.wait().await;
        match tx
            .apply(IdentityWrite::AcceptInvitation {
                invitation_id: invitation,
                user_id: owner,
            })
            .await
        {
            Ok(_) => tx.commit().await.map_err(IdentityError::from),
            Err(e) => {
                tx.rollback().await.unwrap();
                Err(e)
            }
        }
    };
    let (first, second) = tokio::join!(consume(), consume());
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(matches!(
        first.err().or(second.err()),
        Some(IdentityError::Conflict)
    ));
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_invitation WHERE consumed_at IS NOT NULL").await,1);
    assert_eq!(
        scalar(
            &f,
            "SELECT SUM(identity_revision)::bigint AS value FROM \"user\""
        )
        .await,
        1
    );
    f.close().await;
}

#[tokio::test]
async fn identity_account_expired_invitation_rolls_back_created_account_and_subject() {
    use auth::security::budget::Clock;
    use auth::security::identity::SecretToken;
    let f = IdentityFixture::start().await;
    let actor = admin_id(&f).await;
    let invitation = Uuid::new_v4();
    let before = scalar(&f, "SELECT COUNT(*)::bigint AS value FROM \"user\"").await;
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::IssueInvitation(InvitationWrite {
        id: invitation,
        digest: SecretDigest::of("expired-secret"),
        purpose: InvitationPurpose::Onboarding,
        target_user_id: None,
        person_kind: PersonKind::Collaborator,
        developer_grant: false,
        created_by: actor,
        expires_at: f.clock.now().unwrap().unix_millis + 1000,
    }))
    .await
    .unwrap();
    tx.commit().await.unwrap();
    f.clock.advance(1001);
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let IdentityWriteResult::Account(account) = tx
        .apply(IdentityWrite::CreateAccount(NewAccountWrite {
            username: "invited-user".into(),
            password_hash: SecretToken::new("test-hash".into()).unwrap(),
            nickname: "Invited".into(),
            person_kind: PersonKind::Collaborator,
            developer_enabled: false,
        }))
        .await
        .unwrap()
    else {
        panic!("wrong result")
    };
    assert_eq!(
        tx.apply(IdentityWrite::AcceptInvitation {
            invitation_id: invitation,
            user_id: account.user_id
        })
        .await
        .err(),
        Some(IdentityError::Conflict)
    );
    tx.rollback().await.unwrap();
    assert_eq!(
        scalar(&f, "SELECT COUNT(*)::bigint AS value FROM \"user\"").await,
        before
    );
    assert_eq!(
        scalar(&f, "SELECT COUNT(*)::bigint AS value FROM identity_subject").await,
        before
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_invitation WHERE consumed_at IS NOT NULL").await,0);
    f.close().await;
}

#[tokio::test]
async fn identity_account_reset_requires_current_epoch_and_failure_keeps_password() {
    use auth::security::budget::Clock;
    let f = IdentityFixture::start().await;
    let actor = admin_id(&f).await;
    let reset = Uuid::new_v4();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::IssueReset(ResetWrite {
        id: reset,
        user_id: actor,
        digest: SecretDigest::of("reset-proof"),
        auth_epoch_at_issue: 0,
        created_by: actor,
        expires_at: f.clock.now().unwrap().unix_millis + 60_000,
    }))
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    assert_eq!(
        tx.apply(IdentityWrite::ConsumeReset {
            reset_id: reset,
            expected_auth_epoch: 1
        })
        .await
        .err(),
        Some(IdentityError::Conflict)
    );
    tx.rollback().await.unwrap();
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_password_reset WHERE consumed_at IS NOT NULL").await,0);
    f.close().await;
}
