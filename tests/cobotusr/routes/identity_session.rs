//! Stable center roots, revocation, outbox and deadline scenarios.
use super::super::common::IdentityFixture;
use super::identity_support::*;
use auth::security::{budget::Clock, error::SecurityError};
use cobotusr::domain::identity::{application::IdentityScopeSet, error::IdentityError};
use cobotusr::service::identity::repository::*;
use uuid::Uuid;

#[tokio::test]
async fn identity_session_center_bound_manager_can_manage_independent_mcp_and_revoke_scopes() {
    use auth::security::{
        credential::{
            CredentialExpectation, CredentialProfile, CredentialRef, LocalStrictAccessState,
        },
        identity::{PrincipalKind, TokenUse},
    };
    use cobotusr::domain::identity::session::IdentityRevocationScope;
    use cobotusr::service::{
        identity::session::SessionService,
        security::{
            lifecycle::CredentialLifecycle,
            mcp::{McpIssueRequest, McpRotationRequest, McpTokenConfig},
            repository::PgSecurityRepository,
            revocation::RevocationScope,
        },
    };
    use std::sync::Arc;
    let f = IdentityFixture::start().await;
    super::super::common::seed_resource_application(&f.db).await;
    let user_id = admin_id(&f).await;
    let (root, secret) = center_secret(&f, user_id).await;
    let tokens = management_tokens(&f, &root).await;
    let lifecycle = Arc::new(CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(f.db.clone(), f.clock.clone())),
        f.clock.clone(),
    ));
    let state = LocalStrictAccessState::new(
        f.signer.clone(),
        CredentialExpectation::new(
            &f.base_url,
            "cobotusr",
            "cobotusr",
            vec![CredentialProfile::new(PrincipalKind::User, TokenUse::UserSession).unwrap()],
        )
        .unwrap(),
        f.clock.clone(),
        lifecycle.clone(),
    );
    let headers = axum::http::HeaderMap::from_iter([(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", tokens.access_token.expose_secret())
            .parse()
            .unwrap(),
    )]);
    let config = McpTokenConfig {
        issuer: f.base_url.clone(),
        max_ttl_days: 30,
    };
    let manager = state.authenticate(&headers).await.unwrap();
    let mcp = lifecycle
        .issue_mcp(
            &manager,
            McpIssueRequest {
                application: "resource-app",
                name: "independent",
                ttl_days: Some(1),
            },
            &config,
            f.signer.as_ref(),
            &budget(&f),
        )
        .await
        .unwrap();
    let manager = state.authenticate(&headers).await.unwrap();
    lifecycle
        .rename_mcp_token(&manager, mcp.metadata.id, "renamed", &budget(&f))
        .await
        .unwrap();
    let manager = state.authenticate(&headers).await.unwrap();
    let rotated = lifecycle
        .rotate_mcp(
            &manager,
            McpRotationRequest {
                id: mcp.metadata.id,
                ttl_days: Some(1),
            },
            &config,
            f.signer.as_ref(),
            &budget(&f),
        )
        .await
        .unwrap();
    let manager = state.authenticate(&headers).await.unwrap();
    lifecycle
        .delete_mcp_token(&manager, rotated.metadata.id, &budget(&f))
        .await
        .unwrap();
    let manager = state.authenticate(&headers).await.unwrap();
    let independent = lifecycle
        .issue_mcp(
            &manager,
            McpIssueRequest {
                application: "resource-app",
                name: "survives-center",
                ttl_days: Some(1),
            },
            &config,
            f.signer.as_ref(),
            &budget(&f),
        )
        .await
        .unwrap();
    let a = app(&f, "managed-revocation", true).await;
    let (rp_tokens, _, _) = identity_tokens(&f, &a, &root, "openid").await;
    let manager = state.authenticate(&headers).await.unwrap();
    lifecycle
        .revoke_credentials(
            &manager,
            RevocationScope::Credential(
                CredentialRef::new(&f.base_url, &rp_tokens.claims.jti).unwrap(),
            ),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(
        lifecycle
            .check_registered(
                &CredentialRef::new(&f.base_url, &rp_tokens.claims.jti).unwrap(),
                &budget(&f)
            )
            .await
            .is_err()
    );
    SessionService::new(Arc::new(repository(&f)), f.clock.clone())
        .revoke_personal(
            &secret,
            IdentityRevocationScope::CurrentCenter {
                sso_session_id: root.id,
            },
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(state.authenticate(&headers).await.is_err());
    assert!(
        lifecycle
            .check_registered(
                &CredentialRef::new(&f.base_url, &independent.claims.jti).unwrap(),
                &budget(&f)
            )
            .await
            .is_ok()
    );
    f.close().await;
}

#[tokio::test]
async fn identity_session_browser_restore_is_read_only_concurrent_and_epoch_bound() {
    use cobotusr::service::identity::sso::SsoService;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    use std::sync::Arc;
    let f = IdentityFixture::start().await;
    let user_id = admin_id(&f).await;
    let (root, secret) = center_secret(&f, user_id).await;
    let service =
        SsoService::new(Arc::new(repository(&f)), f.clock.clone(), f.config.clone()).unwrap();
    let first_budget = budget(&f);
    let second_budget = budget(&f);
    let (first, second) = tokio::join!(
        service.authenticate_browser(&secret, &first_budget),
        service.authenticate_browser(&secret, &second_budget)
    );
    for restored in [first.unwrap(), second.unwrap()] {
        assert_eq!(restored.root, root);
        assert_eq!(restored.actor.sso_session_id(), root.id);
        assert_eq!(restored.actor.principal().subject(), user_id.to_string());
        assert!(restored.profile.email.is_none());
    }
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_sso_session"
        )
        .await,
        1
    );
    assert_eq!(scalar(&f, "SELECT COUNT(*)::bigint AS value FROM identity_sso_session WHERE last_seen_at IS NOT NULL").await, 0);
    for invalid in ["", "invalid", &"A".repeat(43)] {
        assert_eq!(
            service
                .authenticate_browser(invalid, &budget(&f))
                .await
                .err(),
            Some(IdentityError::Unauthenticated)
        );
    }
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET auth_epoch=auth_epoch+1 WHERE id=$1",
        [user_id.into()],
    ))
    .await
    .unwrap();
    assert_eq!(
        service
            .authenticate_browser(&secret, &budget(&f))
            .await
            .err(),
        Some(IdentityError::Unauthenticated)
    );
    let (_, secret) = center_secret(&f, user_id).await;
    f.clock.advance(43_200_000);
    assert_eq!(
        service
            .authenticate_browser(&secret, &budget(&f))
            .await
            .err(),
        Some(IdentityError::Unauthenticated)
    );
    f.close().await;
}

#[tokio::test]
async fn identity_session_personal_revocation_locks_full_scope_and_keeps_other_browser_and_user() {
    use auth::security::{credential::CredentialRef, identity::SecretToken};
    use cobotusr::domain::identity::{account::PersonKind, session::IdentityRevocationScope};
    use cobotusr::service::{
        identity::session::SessionService,
        security::{lifecycle::CredentialLifecycle, repository::PgSecurityRepository},
    };
    use std::sync::Arc;
    let f = IdentityFixture::start().await;
    let a = app(&f, "revocation-web", true).await;
    let b = app(&f, "revocation-spa", true).await;
    let (first_root, first_secret) = center_secret(&f, a.owner).await;
    let (other_browser, other_secret) = center_secret(&f, a.owner).await;
    let (first, _, _) = identity_tokens(&f, &a, &first_root, "openid").await;
    let (second, _, _) = identity_tokens(&f, &b, &first_root, "openid").await;
    let (other, _, _) = identity_tokens(&f, &a, &other_browser, "openid").await;
    let mut transaction = repository(&f).begin(&budget(&f)).await.unwrap();
    let IdentityWriteResult::Account(new_user) = transaction
        .apply(IdentityWrite::CreateAccount(NewAccountWrite {
            username: "revocation-other-person".into(),
            password_hash: SecretToken::new("fixture-hash".into()).unwrap(),
            nickname: "Other".into(),
            person_kind: PersonKind::Employee,
            developer_enabled: false,
        }))
        .await
        .unwrap()
    else {
        panic!("account expected")
    };
    transaction
        .apply(IdentityWrite::SetTestUser {
            environment_id: a.development,
            user_id: new_user.user_id,
            present: true,
            actor_id: a.owner,
        })
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    let (other_person_root, _) = center_secret(&f, new_user.user_id).await;
    let (other_person, _, _) = identity_tokens(&f, &a, &other_person_root, "openid").await;
    let service = SessionService::new(Arc::new(repository(&f)), f.clock.clone());
    assert_eq!(
        service
            .revoke_personal(
                &first_secret,
                IdentityRevocationScope::CurrentCenter {
                    sso_session_id: other_person_root.id
                },
                Uuid::new_v4(),
                &budget(&f)
            )
            .await,
        Err(IdentityError::Forbidden)
    );
    service
        .revoke_personal(
            &first_secret,
            IdentityRevocationScope::CurrentCenter {
                sso_session_id: first_root.id,
            },
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    let lifecycle = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(f.db.clone(), f.clock.clone())),
        f.clock.clone(),
    );
    for (set, active) in [
        (&first, false),
        (&second, false),
        (&other, true),
        (&other_person, true),
    ] {
        assert_eq!(
            lifecycle
                .check_registered(
                    &CredentialRef::new(&f.base_url, &set.claims.jti).unwrap(),
                    &budget(&f)
                )
                .await
                .is_ok(),
            active
        );
    }
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery"
        )
        .await,
        2
    );
    assert_eq!(scalar(&f, "SELECT COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='session.revoke' AND result='success'").await, 1);
    service
        .revoke_personal(
            &other_secret,
            IdentityRevocationScope::UserAll { user_id: a.owner },
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    assert!(
        lifecycle
            .check_registered(
                &CredentialRef::new(&f.base_url, &other.claims.jti).unwrap(),
                &budget(&f)
            )
            .await
            .is_err()
    );
    assert!(
        lifecycle
            .check_registered(
                &CredentialRef::new(&f.base_url, &other_person.claims.jti).unwrap(),
                &budget(&f)
            )
            .await
            .is_ok()
    );
    f.close().await;
}

#[tokio::test]
async fn identity_session_registered_managed_access_observes_center_revocation_and_epoch_changes() {
    use auth::security::credential::CredentialRef;
    use cobotusr::domain::identity::session::IdentityRevocationScope;
    use cobotusr::service::security::{
        lifecycle::CredentialLifecycle, repository::PgSecurityRepository,
    };
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    use std::sync::Arc;
    let f = IdentityFixture::start().await;
    let a = app(&f, "root-liveness", false).await;
    let root = center(&f, a.owner).await;
    let (set, _, _) = identity_tokens(&f, &a, &root, "openid").await;
    let lifecycle = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(f.db.clone(), f.clock.clone())),
        f.clock.clone(),
    );
    let reference = CredentialRef::new(&f.base_url, &set.claims.jti).unwrap();
    assert!(
        lifecycle
            .check_registered(&reference, &budget(&f))
            .await
            .unwrap()
            .active
    );
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::Revoke(
        IdentityRevocationScope::CurrentCenter {
            sso_session_id: root.id,
        },
    ))
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        lifecycle
            .check_registered(&reference, &budget(&f))
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
    let root = center(&f, a.owner).await;
    let (set, _, _) = identity_tokens(&f, &a, &root, "openid").await;
    let reference = CredentialRef::new(&f.base_url, &set.claims.jti).unwrap();
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET auth_epoch=auth_epoch+1 WHERE id=$1",
        [a.owner.into()],
    ))
    .await
    .unwrap();
    // No per-credential revoked flag was set; current epoch still rejects it.
    assert_eq!(
        lifecycle
            .check_registered(&reference, &budget(&f))
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
    f.close().await;
}

#[tokio::test]
async fn identity_session_registered_access_never_revives_after_final_source_loss_or_regrant() {
    use auth::security::credential::CredentialRef;
    use cobotusr::service::security::{
        lifecycle::CredentialLifecycle, repository::PgSecurityRepository,
    };
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    use std::sync::Arc;
    let f = IdentityFixture::start().await;
    let a = app(&f, "generation-liveness", false).await;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE application SET management_mode='developer_managed' WHERE id=$1",
        [a.app.into()],
    ))
    .await
    .unwrap();
    let root = center(&f, a.owner).await;
    let (old, _, _) = identity_tokens(&f, &a, &root, "openid").await;
    let lifecycle = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(f.db.clone(), f.clock.clone())),
        f.clock.clone(),
    );
    let reference = CredentialRef::new(&f.base_url, &old.claims.jti).unwrap();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let mut account = tx.load_account(a.owner).await.unwrap().unwrap();
    account.developer_enabled = false;
    tx.apply(IdentityWrite::SetAccount {
        account,
        expected_revision: 0,
    })
    .await
    .unwrap();
    tx.apply(IdentityWrite::RecomputeGeneration {
        environment_id: a.development,
        user_id: a.owner,
    })
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        lifecycle
            .check_registered(&reference, &budget(&f))
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let mut account = tx.load_account(a.owner).await.unwrap().unwrap();
    account.developer_enabled = true;
    tx.apply(IdentityWrite::SetAccount {
        account,
        expected_revision: 1,
    })
    .await
    .unwrap();
    tx.apply(IdentityWrite::RecomputeGeneration {
        environment_id: a.development,
        user_id: a.owner,
    })
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        lifecycle
            .check_registered(&reference, &budget(&f))
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
    let (fresh, _, _) = identity_tokens(&f, &a, &root, "openid").await;
    assert!(
        lifecycle
            .check_registered(
                &CredentialRef::new(&f.base_url, &fresh.claims.jti).unwrap(),
                &budget(&f)
            )
            .await
            .unwrap()
            .active
    );
    f.close().await;
}

struct RetryingGroup {
    id: Uuid,
    actor: i32,
    calls: std::sync::atomic::AtomicUsize,
    conflicts: usize,
    advance: u64,
    clock: std::sync::Arc<super::super::common::SecurityTestClock>,
}

#[sea_orm::prelude::async_trait::async_trait]
impl cobotusr::service::identity::transaction::IdentityUnitOfWork for RetryingGroup {
    type Output = usize;
    async fn apply(&self, tx: &mut dyn IdentityTransaction) -> Result<usize, IdentityError> {
        let call = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        tx.apply(IdentityWrite::CreateGroup {
            id: self.id,
            name: "transaction-retry".into(),
            description: None,
            actor_id: self.actor,
        })
        .await?;
        self.clock.advance(self.advance);
        if call <= self.conflicts {
            Err(IdentityError::Conflict)
        } else {
            Ok(call)
        }
    }
}

#[tokio::test]
async fn identity_session_whole_transaction_retry_rolls_back_each_attempt_and_stops_at_three() {
    use cobotusr::service::identity::transaction::execute_identity;
    let f = IdentityFixture::start().await;
    let actor = admin_id(&f).await;
    let first = RetryingGroup {
        id: Uuid::new_v4(),
        actor,
        calls: Default::default(),
        conflicts: 2,
        advance: 0,
        clock: f.clock.clone(),
    };
    assert_eq!(
        execute_identity(&repository(&f), &budget(&f), &first).await,
        Ok(3)
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_user_group"
        )
        .await,
        1
    );
    let exhausted = RetryingGroup {
        id: Uuid::new_v4(),
        actor,
        calls: Default::default(),
        conflicts: 99,
        advance: 0,
        clock: f.clock.clone(),
    };
    assert_eq!(
        execute_identity(&repository(&f), &budget(&f), &exhausted).await,
        Err(IdentityError::Conflict)
    );
    assert_eq!(exhausted.calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_user_group"
        )
        .await,
        1
    );
    f.close().await;
}

#[tokio::test]
async fn identity_session_retries_keep_the_original_deadline() {
    use cobotusr::service::identity::transaction::execute_identity;
    let f = IdentityFixture::start().await;
    let unit = RetryingGroup {
        id: Uuid::new_v4(),
        actor: admin_id(&f).await,
        calls: Default::default(),
        conflicts: 99,
        advance: 1_100,
        clock: f.clock.clone(),
    };
    assert_eq!(
        execute_identity(&repository(&f), &budget(&f), &unit).await,
        Err(IdentityError::DeadlineExceeded)
    );
    assert_eq!(unit.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_user_group"
        )
        .await,
        0
    );
    f.close().await;
}

struct ReceiptUnit {
    actor: i32,
    operation: Uuid,
    calls: std::sync::atomic::AtomicUsize,
}
#[sea_orm::prelude::async_trait::async_trait]
impl cobotusr::service::identity::transaction::IdentityUnitOfWork for ReceiptUnit {
    type Output = auth::security::identity::SecretToken;
    async fn apply(&self, tx: &mut dyn IdentityTransaction) -> Result<Self::Output, IdentityError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tx.apply(IdentityWrite::SaveOperation(OperationReceiptWrite {
            id: self.operation,
            actor_id: self.actor,
            operation_kind: "invitation.create",
            idempotency_key: "uncertain-commit".into(),
            request_hash: "a".repeat(64),
            resource_id: self.operation.to_string(),
            summary: serde_json::json!({"status":"created"}),
        }))
        .await?;
        Ok(auth::security::identity::SecretToken::new(
            "never-replay-this-response".into(),
        )?)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn identity_session_uncertain_commit_returns_no_secret_and_receipt_can_be_queried() {
    use cobotusr::service::identity::transaction::execute_identity;
    use sea_orm::ConnectionTrait;
    use std::{
        sync::{Arc, atomic::Ordering},
        time::Duration,
    };
    let f = IdentityFixture::start().await;
    f.db.execute_unprepared("CREATE FUNCTION identity_commit_gate() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_sleep(0.3); RETURN NEW; END $$").await.unwrap();
    f.db.execute_unprepared("CREATE CONSTRAINT TRIGGER identity_commit_gate AFTER INSERT ON identity_request_operation DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION identity_commit_gate()").await.unwrap();
    let unit = Arc::new(ReceiptUnit {
        actor: admin_id(&f).await,
        operation: Uuid::new_v4(),
        calls: Default::default(),
    });
    let repo = Arc::new(repository(&f));
    let operation_budget = budget(&f);
    let job = {
        let (repo, unit) = (repo.clone(), unit.clone());
        tokio::spawn(async move {
            execute_identity(repo.as_ref(), &operation_budget, unit.as_ref()).await
        })
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            assert!(!job.is_finished(), "operation ended before reaching the deferred commit gate");
            if scalar(&f, "SELECT COUNT(*)::bigint AS value FROM pg_stat_activity WHERE datname=current_database() AND state='active' AND query='COMMIT' AND wait_event='PgSleep'").await > 0 { break; }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.unwrap();
    f.clock.advance(2_001);
    assert_eq!(
        job.await.unwrap().err(),
        Some(IdentityError::CommitUncertain)
    );
    assert_eq!(unit.calls.load(Ordering::SeqCst), 1);
    let mut tx = repo.begin(&budget(&f)).await.unwrap();
    let receipt = tx
        .operation_receipt(unit.actor, "invitation.create", "uncertain-commit")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.operation_id, unit.operation);
    assert!(
        !serde_json::to_string(&receipt)
            .unwrap()
            .contains("never-replay")
    );
    tx.commit().await.unwrap();
    f.close().await;
}

#[tokio::test]
async fn identity_session_locks_reject_late_inverse_scope_and_allow_repeat() {
    let f = IdentityFixture::start().await;
    let a = app(&f, "locks", false).await;
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    tx.lock_identity_scope(
        &IdentityLockScope::new(vec![
            IdentityLockKey::User(a.owner),
            IdentityLockKey::Environment(a.development),
            IdentityLockKey::Application(a.app),
        ])
        .unwrap(),
    )
    .await
    .unwrap();
    tx.lock_identity_scope(
        &IdentityLockScope::new(vec![
            IdentityLockKey::User(a.owner),
            IdentityLockKey::Application(a.app),
        ])
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        tx.lock_identity_scope(
            &IdentityLockScope::new(vec![IdentityLockKey::Environment(a.production)]).unwrap()
        )
        .await,
        Err(IdentityError::Conflict)
    );
    tx.rollback().await.unwrap();
    f.close().await;
}

#[tokio::test]
async fn identity_session_shared_transaction_rolls_back_browser_security_pair_and_audit() {
    use cobotusr::domain::identity::{
        account::{SecretDigest, stable_principal},
        session::BrowserSessionSnapshot,
    };
    use cobotusr::service::security::{
        lifecycle::{CredentialLifecycle, LocalLoginRequest, LocalPassword, SessionTokenConfig},
        repository::PgSecurityRepository,
    };
    use std::sync::Arc;
    let f = IdentityFixture::start().await;
    let owner = admin_id(&f).await;
    let lifecycle = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(f.db.clone(), f.clock.clone())),
        f.clock.clone(),
    );
    let request_budget = budget(&f);
    let mut tx = repository(&f).begin(&request_budget).await.unwrap();
    let login = lifecycle
        .login_local_in_transaction(
            tx.as_mut(),
            LocalLoginRequest {
                username: "identity-admin",
                password: &LocalPassword::new("fixture-admin-password".into()).unwrap(),
            },
            &SessionTokenConfig {
                issuer: f.base_url.clone(),
                access_ttl_seconds: 300,
                refresh_ttl_seconds: 3600,
            },
            f.signer.as_ref(),
        )
        .await
        .unwrap();
    let root_id = Uuid::new_v4();
    let now = f.clock.now().unwrap().unix_millis;
    tx.apply(IdentityWrite::CreateCenter(BrowserSessionSnapshot {
        id: root_id,
        principal: stable_principal(&f.base_url, owner).unwrap(),
        user_id: owner,
        secret_digest: SecretDigest::of("test-browser-root"),
        auth_epoch: 0,
        authenticated_at: now,
        expires_at: now + 43_200_000,
        revoked_at: None,
    }))
    .await
    .unwrap();
    tx.apply(IdentityWrite::AttachManagementSession {
        session_id: Uuid::parse_str(login.tokens.claims.session_id.as_ref().unwrap()).unwrap(),
        sso_session_id: root_id,
    })
    .await
    .unwrap();
    tx.append_audit(&AuditEvent {
        id: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        actor_id: Some(owner),
        action: "login",
        object_kind: "root",
        object_id: root_id.to_string(),
        result: AuditResult::Success,
        reason: None,
        change_summary: serde_json::json!({}),
        operation_id: None,
        commit_marker: Some(Uuid::new_v4()),
    })
    .await
    .unwrap();
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_sso_session"
        )
        .await,
        0
    );
    tx.rollback().await.unwrap();
    for table in [
        "identity_sso_session",
        "security_session",
        "security_grant",
        "security_refresh_family",
        "security_credential",
        "identity_audit_event",
    ] {
        assert_eq!(
            scalar(
                &f,
                &format!("SELECT COUNT(*)::bigint AS value FROM {table}")
            )
            .await,
            0,
            "orphan in {table}"
        );
    }
    f.close().await;
}

#[tokio::test]
async fn identity_session_deadline_before_commit_cannot_commit_a_root() {
    let f = IdentityFixture::start().await;
    let owner = admin_id(&f).await;
    let request_budget = budget(&f);
    let mut tx = repository(&f).begin(&request_budget).await.unwrap();
    tx.apply(IdentityWrite::CreateGroup {
        id: Uuid::new_v4(),
        name: "expires".into(),
        description: None,
        actor_id: owner,
    })
    .await
    .unwrap();
    f.clock.advance(2001);
    assert_eq!(tx.commit().await, Err(SecurityError::DeadlineExceeded));
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_user_group"
        )
        .await,
        0
    );
    f.close().await;
}

#[tokio::test]
async fn identity_session_outbox_is_idempotent_and_crash_leases_can_be_reclaimed() {
    use cobotusr::domain::identity::session::IdentityRevocationScope;
    use cobotusr::service::identity::repository_outbox::DeliveryOutcome;
    let f = IdentityFixture::start().await;
    let a = app(&f, "outbox", true).await;
    let root = center(&f, a.owner).await;
    let repo = repository(&f);
    let mut tx = repo.begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::RecomputeGeneration {
        environment_id: a.development,
        user_id: a.owner,
    })
    .await
    .unwrap();
    let admission = tx
        .evaluate_admission(&AdmissionQuery {
            sso_session_id: root.id,
            environment_id: a.development,
            requested_scopes: IdentityScopeSet::parse("openid").unwrap(),
        })
        .await
        .unwrap();
    let rp = Uuid::new_v4();
    tx.apply(IdentityWrite::CreateRelyingParty {
        id: rp,
        protocol_sid: Uuid::new_v4(),
        admission,
        nonce: Some("test-nonce".into()),
    })
    .await
    .unwrap();
    tx.apply(IdentityWrite::Revoke(
        IdentityRevocationScope::CurrentCenter {
            sso_session_id: root.id,
        },
    ))
    .await
    .unwrap();
    let event = Uuid::new_v4();
    tx.append_audit(&AuditEvent {
        id: event,
        request_id: Uuid::new_v4(),
        actor_id: Some(a.owner),
        action: "logout",
        object_kind: "root",
        object_id: root.id.to_string(),
        result: AuditResult::Success,
        reason: None,
        change_summary: serde_json::json!({}),
        operation_id: None,
        commit_marker: Some(Uuid::new_v4()),
    })
    .await
    .unwrap();
    for _ in 0..2 {
        tx.enqueue_logout(&LogoutEvent {
            audit_event_id: event,
            rp_session_ids: vec![rp],
        })
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery"
        )
        .await,
        1
    );
    let worker1 = Uuid::new_v4();
    let worker2 = Uuid::new_v4();
    let leased = repo.lease_logout(&budget(&f), worker1, 10).await.unwrap();
    assert_eq!(leased.len(), 1);
    assert!(
        repo.lease_logout(&budget(&f), worker2, 10)
            .await
            .unwrap()
            .is_empty()
    );
    f.clock.advance(30_001);
    let reclaimed = repo.lease_logout(&budget(&f), worker2, 10).await.unwrap();
    assert_eq!(reclaimed.len(), 1);
    assert_eq!(reclaimed[0].token_jti, leased[0].token_jti);
    assert_eq!(
        repo.finish_logout(
            &budget(&f),
            leased[0].delivery_id,
            worker1,
            DeliveryOutcome::Delivered
        )
        .await,
        Err(IdentityError::Conflict)
    );
    repo.finish_logout(
        &budget(&f),
        reclaimed[0].delivery_id,
        worker2,
        DeliveryOutcome::Retry {
            error_code: "timeout",
        },
    )
    .await
    .unwrap();
    assert!(
        repo.lease_logout(&budget(&f), worker1, 10)
            .await
            .unwrap()
            .is_empty()
    );
    f.clock.advance(30_001);
    let final_lease = repo.lease_logout(&budget(&f), worker1, 10).await.unwrap();
    assert_eq!(final_lease.len(), 1);
    repo.finish_logout(
        &budget(&f),
        final_lease[0].delivery_id,
        worker1,
        DeliveryOutcome::Delivered,
    )
    .await
    .unwrap();
    assert!(
        repo.lease_logout(&budget(&f), worker2, 10)
            .await
            .unwrap()
            .is_empty()
    );
    f.close().await;
}
