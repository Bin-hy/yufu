use super::*;
use auth::security::{
    credential::{
        AuthenticatedContext, CredentialExpectation, CredentialProfile, CredentialRef,
        StrictAccessState,
    },
    error::SecurityError,
    identity::{PrincipalKind, TokenUse},
    policy::{
        OperationId, ResourceKind, ResourceRef, ResourceScope, ResourceSelector, ResourceTarget,
    },
};
use axum::http;
use cobotusr::service::security::{
    lifecycle::{CredentialLifecycle, SessionTokenSigner},
    mcp::{McpIssueRequest, McpRotationRequest, McpTokenConfig, McpTokenSet},
    revocation::RevocationScope,
};
use sea_orm::EntityTrait;
use std::sync::Arc;

async fn seed_mcp(db: &DatabaseConnection) {
    crate::cobotusr::common::seed_resource_application(db).await;
    let access = serde_json::to_string(
        &ResourceSelector::new(
            "resource-app",
            ResourceScope::Application {
                kind: ResourceKind::Application,
            },
        )
        .unwrap(),
    )
    .unwrap();
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres, "UPDATE permission SET action='mcp:access',resource_type='application',resource_pattern=$1 WHERE name='mcp:access' AND application_id=(SELECT id FROM application WHERE app_key='resource-app')", [access.into()])).await.unwrap();
    let scope = serde_json::to_string(
        &ResourceSelector::new(
            "resource-app",
            ResourceScope::Gateway {
                gateway_id: "g1".into(),
                kind: ResourceKind::Device,
            },
        )
        .unwrap(),
    )
    .unwrap();
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres, "INSERT INTO permission (application_id,name,action,resource_type,resource_pattern,create_time,update_time) SELECT id,'t26-device','device:read','device',$1,NOW(),NOW() FROM application WHERE app_key='resource-app'", [scope.into()])).await.unwrap();
    execute(db,"INSERT INTO role_permission (role_id,permission_id,create_time) SELECT 3,p.id,NOW() FROM permission p WHERE p.name='t26-device'").await.unwrap();
}
async fn context(
    claims: &auth::security::credential::SecurityAccessClaims,
    token: &auth::security::identity::SecretToken,
    lifecycle: Arc<CredentialLifecycle>,
    clock: Arc<crate::cobotusr::common::SecurityTestClock>,
) -> AuthenticatedContext {
    let (jwks, _stop) = security_jwks().await;
    StrictAccessState::new(
        jwks,
        CredentialExpectation::new(
            &claims.iss,
            &claims.aud,
            &claims.application,
            vec![CredentialProfile::new(PrincipalKind::User, claims.token_use).unwrap()],
        )
        .unwrap(),
        clock,
        lifecycle,
    )
    .authenticate(&http::HeaderMap::from_iter([(
        http::header::AUTHORIZATION,
        format!("Bearer {}", token.expose_secret()).parse().unwrap(),
    )]))
    .await
    .unwrap()
}
fn config() -> McpTokenConfig {
    McpTokenConfig {
        issuer: session_config().issuer,
        max_ttl_days: 90,
    }
}
async fn issue(
    lifecycle: &CredentialLifecycle,
    manager: &AuthenticatedContext,
    signer: &dyn SessionTokenSigner,
) -> Result<McpTokenSet, SecurityError> {
    lifecycle
        .issue_mcp(
            manager,
            McpIssueRequest {
                application: "resource-app",
                name: "personal MCP",
                ttl_days: Some(9999),
            },
            &config(),
            signer,
            manager.budget(),
        )
        .await
}
async fn active(
    lifecycle: &CredentialLifecycle,
    set: &McpTokenSet,
    clock: &Arc<crate::cobotusr::common::SecurityTestClock>,
) {
    lifecycle
        .check_registered(
            &CredentialRef::new(&set.claims.iss, &set.claims.jti).unwrap(),
            &security_budget(clock),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn security_t26_personal_management_mints_registered_independent_bounded_mcp() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    seed_mcp(&db).await;
    execute(&db, "UPDATE \"user\" SET role_id=3 WHERE id=1")
        .await
        .unwrap();
    let signer = SessionTestSigner::new();
    let session = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let manager = context(
        &session.claims,
        &session.access_token,
        lifecycle.clone(),
        clock.clone(),
    )
    .await;
    let first = issue(&lifecycle, &manager, &signer).await.unwrap();
    let second = issue(&lifecycle, &manager, &signer).await.unwrap();
    assert_eq!(first.claims.token_use, TokenUse::McpAccess);
    assert_eq!(first.claims.aud, "resource-app");
    assert_eq!(first.claims.exp - first.claims.iat, 90 * 86_400);
    assert!(first.claims.session_id.is_none() && first.claims.family_id.is_none());
    assert_ne!(first.claims.grant_id, session.claims.grant_id);
    assert_ne!(first.claims.grant_id, second.claims.grant_id);
    let op = OperationId::new("device:read").unwrap();
    for (gateway, expected) in [("g1", true), ("g2", false)] {
        assert_eq!(
            first.claims.bounds.allows(
                &op,
                &ResourceRef::new(
                    "resource-app",
                    ResourceTarget::Device {
                        gateway_id: gateway.into(),
                        device_id: "d".into()
                    }
                )
                .unwrap()
            ),
            expected
        );
    }
    let row = cobotusr::entity::security_credential::Entity::find_by_id((
        first.claims.iss.clone(),
        first.claims.jti.clone(),
    ))
    .one(&db)
    .await
    .unwrap()
    .unwrap();
    assert!(row.revocation_parent_jti.is_none() && row.secret_digest.is_none());
    assert_eq!(row.minted_by.unwrap()["jti"], session.claims.jti);
    active(&lifecycle, &first, &clock).await;
    lifecycle
        .logout_local(
            &session.refresh_token,
            &session.claims.iss,
            None,
            &security_budget(&clock),
        )
        .await
        .unwrap();
    active(&lifecycle, &first, &clock).await;
    active(&lifecycle, &second, &clock).await;
}

#[tokio::test]
async fn security_t26_missing_old_target_bounds_current_deny_and_mcp_manager_cannot_mint() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    execute(&db,"UPDATE permission SET action='access',resource_type='mcp',resource_pattern='*' WHERE name='mcp:access'").await.unwrap();
    let signer = SessionTestSigner::new();
    let old = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let old_manager = context(
        &old.claims,
        &old.access_token,
        lifecycle.clone(),
        clock.clone(),
    )
    .await;
    seed_mcp(&db).await;
    assert_eq!(
        issue(&lifecycle, &old_manager, &signer).await.err(),
        Some(SecurityError::Forbidden)
    );
    let session = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let manager = context(
        &session.claims,
        &session.access_token,
        lifecycle.clone(),
        clock.clone(),
    )
    .await;
    let set = issue(&lifecycle, &manager, &signer).await.unwrap();
    let mcp = context(
        &set.claims,
        &set.access_token,
        lifecycle.clone(),
        clock.clone(),
    )
    .await;
    assert_eq!(
        issue(&lifecycle, &mcp, &signer).await.err(),
        Some(SecurityError::Forbidden)
    );
    execute(&db,"INSERT INTO user_permission (user_id,permission_id,allow,create_time,update_time) SELECT 1,id,FALSE,NOW(),NOW() FROM permission WHERE name='mcp:access'").await.unwrap();
    assert_eq!(
        issue(&lifecycle, &manager, &signer).await.err(),
        Some(SecurityError::Forbidden)
    );
    execute(&db, "DELETE FROM user_permission WHERE user_id=1")
        .await
        .unwrap();
    lifecycle
        .logout_local(
            &session.refresh_token,
            &session.claims.iss,
            None,
            &security_budget(&clock),
        )
        .await
        .unwrap();
    assert_eq!(
        issue(&lifecycle, &manager, &signer).await.err(),
        Some(SecurityError::CredentialRevoked)
    );
    assert_eq!(session_count(&db, "mcp_token").await, 1);
}

#[tokio::test]
async fn security_t26_signing_commit_and_inherited_budget_failures_leave_no_mcp() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    seed_mcp(&db).await;
    let signer = SessionTestSigner::new();
    let session = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let manager = context(
        &session.claims,
        &session.access_token,
        lifecycle.clone(),
        clock.clone(),
    )
    .await;
    signer.fail.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        issue(&lifecycle, &manager, &signer).await.err(),
        Some(SecurityError::DependencyUnavailable)
    );
    signer
        .fail
        .store(false, std::sync::atomic::Ordering::SeqCst);
    execute(&db,"CREATE FUNCTION reject_mcp_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test commit rejected'; END $$").await.unwrap();
    execute(&db,"CREATE CONSTRAINT TRIGGER reject_mcp_commit AFTER INSERT ON mcp_token DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_mcp_commit()").await.unwrap();
    assert_eq!(
        issue(&lifecycle, &manager, &signer).await.err(),
        Some(SecurityError::DependencyUnavailable)
    );
    execute(&db, "DROP TRIGGER reject_mcp_commit ON mcp_token")
        .await
        .unwrap();
    let slow = SessionTestSigner {
        delay: std::time::Duration::from_millis(300),
        ..SessionTestSigner::new()
    };
    let budget = manager
        .budget()
        .child(std::time::Duration::from_millis(100))
        .unwrap();
    assert_eq!(
        lifecycle
            .issue_mcp(
                &manager,
                McpIssueRequest {
                    application: "resource-app",
                    name: "slow",
                    ttl_days: None
                },
                &config(),
                &slow,
                &budget
            )
            .await
            .err(),
        Some(SecurityError::DeadlineExceeded)
    );
    assert_eq!(session_count(&db, "mcp_token").await, 0);
    assert_eq!(
        session_count(&db, "security_grant WHERE grant_kind='mcp'").await,
        0
    );
    assert_eq!(
        session_count(&db, "security_credential WHERE token_use='mcp_access'").await,
        0
    );
    // A caller-supplied fresh budget cannot restart the authenticated request.
    clock.advance(1900);
    assert_eq!(
        lifecycle
            .issue_mcp(
                &manager,
                McpIssueRequest {
                    application: "resource-app",
                    name: "fresh budget",
                    ttl_days: None
                },
                &config(),
                &slow,
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::DeadlineExceeded)
    );
    assert_eq!(session_count(&db, "mcp_token").await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn security_t26_parallel_new_roots_cannot_exceed_quota() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    seed_mcp(&db).await;
    let signer = Arc::new(SessionTestSigner::new());
    let session = login_local_session(&lifecycle, signer.as_ref(), &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let manager = Arc::new(
        context(
            &session.claims,
            &session.access_token,
            lifecycle.clone(),
            clock.clone(),
        )
        .await,
    );
    for _ in 0..19 {
        issue(&lifecycle, &manager, signer.as_ref()).await.unwrap();
    }
    let barrier = Arc::new(tokio::sync::Barrier::new(8));
    let mut jobs = Vec::new();
    for _ in 0..8 {
        let (lifecycle, manager, signer, barrier) = (
            lifecycle.clone(),
            manager.clone(),
            signer.clone(),
            barrier.clone(),
        );
        jobs.push(tokio::spawn(async move {
            barrier.wait().await;
            issue(&lifecycle, &manager, signer.as_ref()).await
        }));
    }
    let mut successes = 0;
    for job in jobs {
        match job.await.unwrap() {
            Ok(_) => successes += 1,
            Err(error) => assert!(matches!(
                error,
                SecurityError::Conflict | SecurityError::CapacityExceeded
            )),
        }
    }
    assert_eq!(successes, 1);
    assert_eq!(
        session_count(&db, "mcp_token WHERE revoked=FALSE").await,
        20
    );
    assert_eq!(
        session_count(&db, "security_grant WHERE grant_kind='mcp'").await,
        20
    );
}

async fn rotate(
    lifecycle: &CredentialLifecycle,
    manager: &AuthenticatedContext,
    id: cobotusr::IdType,
    signer: &dyn SessionTokenSigner,
) -> Result<McpTokenSet, SecurityError> {
    lifecycle
        .rotate_mcp(
            manager,
            McpRotationRequest {
                id,
                ttl_days: Some(7),
            },
            &config(),
            signer,
            manager.budget(),
        )
        .await
}

#[tokio::test]
async fn security_t27_rotation_retains_root_ceiling_and_revokes_explicit_descendants() {
    use cobotusr::entity::security_credential as credential;
    use sea_orm::{ActiveModelTrait, Set};
    let (_container, db, clock, lifecycle) = local_fixture().await;
    seed_mcp(&db).await;
    let signer = SessionTestSigner::new();
    let session = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let manager = context(
        &session.claims,
        &session.access_token,
        lifecycle.clone(),
        clock.clone(),
    )
    .await;
    let old = issue(&lifecycle, &manager, &signer).await.unwrap();
    let row = credential::Entity::find_by_id((old.claims.iss.clone(), old.claims.jti.clone()))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut child: credential::ActiveModel = row.clone().into();
    child.jti = Set("mcp-delegated".into());
    child.token_use = Set("gateway_operation".into());
    child.audience = Set("legacy-gateway:g1".into());
    child.binding = Set(serde_json::json!({"kind":"gateway","gateway_id":"g1"}));
    child.revocation_parent_jti = Set(Some(old.claims.jti.clone()));
    child.insert(&db).await.unwrap();
    // Current role expansion and a newly logged-in manager cannot expand the old MCP.
    let broad = serde_json::to_string(
        &ResourceSelector::new(
            "resource-app",
            ResourceScope::Application {
                kind: ResourceKind::Device,
            },
        )
        .unwrap(),
    )
    .unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE permission SET resource_pattern=$1 WHERE name='t26-device'",
        [broad.into()],
    ))
    .await
    .unwrap();
    let expanded = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let manager = context(
        &expanded.claims,
        &expanded.access_token,
        lifecycle.clone(),
        clock.clone(),
    )
    .await;
    let new = rotate(&lifecycle, &manager, old.metadata.id, &signer)
        .await
        .unwrap();
    assert_eq!(new.claims.grant_id, old.claims.grant_id);
    assert_ne!(new.claims.jti, old.claims.jti);
    assert!(new.claims.session_id.is_none() && new.claims.family_id.is_none());
    for (gateway, expected) in [("g1", true), ("g2", false)] {
        assert_eq!(
            new.claims.bounds.allows(
                &OperationId::new("device:read").unwrap(),
                &ResourceRef::new(
                    "resource-app",
                    ResourceTarget::Device {
                        gateway_id: gateway.into(),
                        device_id: "d".into()
                    }
                )
                .unwrap()
            ),
            expected
        );
    }
    for jti in [&old.claims.jti, "mcp-delegated"] {
        assert_eq!(
            lifecycle
                .check_registered(
                    &CredentialRef::new(&old.claims.iss, jti).unwrap(),
                    &security_budget(&clock)
                )
                .await
                .err(),
            Some(SecurityError::CredentialRevoked)
        );
    }
    active(&lifecycle, &new, &clock).await;
    lifecycle
        .revoke_credentials(
            &manager,
            RevocationScope::Credential(
                CredentialRef::new(&old.claims.iss, &old.claims.jti).unwrap(),
            ),
            &security_budget(&clock),
        )
        .await
        .unwrap();
    active(&lifecycle, &new, &clock).await;
    let row = credential::Entity::find_by_id((new.claims.iss.clone(), new.claims.jti.clone()))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(row.revocation_parent_jti.is_none());
    assert_eq!(row.minted_by.unwrap()["jti"], expanded.claims.jti);
    assert_eq!(
        rotate(&lifecycle, &manager, old.metadata.id, &signer)
            .await
            .err(),
        Some(SecurityError::Conflict)
    );
    active(&lifecycle, &new, &clock).await;
    lifecycle
        .revoke_credentials(
            &manager,
            RevocationScope::ApplicationGrants {
                application: "resource-app".into(),
            },
            &security_budget(&clock),
        )
        .await
        .unwrap();
    assert_eq!(
        lifecycle
            .check_registered(
                &CredentialRef::new(&new.claims.iss, &new.claims.jti).unwrap(),
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
    assert_eq!(session_count(&db, "mcp_token WHERE revoked=FALSE").await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn security_t27_parallel_rotation_commits_exactly_one_replacement() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    seed_mcp(&db).await;
    let signer = Arc::new(SessionTestSigner::new());
    let session = login_local_session(&lifecycle, signer.as_ref(), &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let manager = Arc::new(
        context(
            &session.claims,
            &session.access_token,
            lifecycle.clone(),
            clock.clone(),
        )
        .await,
    );
    let old = issue(&lifecycle, &manager, signer.as_ref()).await.unwrap();
    let barrier = Arc::new(tokio::sync::Barrier::new(8));
    let mut jobs = Vec::new();
    for _ in 0..8 {
        let (lifecycle, manager, signer, barrier) = (
            lifecycle.clone(),
            manager.clone(),
            signer.clone(),
            barrier.clone(),
        );
        let id = old.metadata.id;
        jobs.push(tokio::spawn(async move {
            barrier.wait().await;
            rotate(&lifecycle, &manager, id, signer.as_ref()).await
        }));
    }
    let mut successes = Vec::new();
    for job in jobs {
        match job.await.unwrap() {
            Ok(set) => successes.push(set),
            Err(e) => assert_eq!(e, SecurityError::Conflict),
        }
    }
    assert_eq!(successes.len(), 1);
    active(&lifecycle, &successes[0], &clock).await;
    assert_eq!(
        session_count(&db, "security_grant WHERE grant_kind='mcp'").await,
        1
    );
    assert_eq!(session_count(&db, "mcp_token").await, 2);
    assert_eq!(session_count(&db, "mcp_token WHERE revoked=FALSE").await, 1);
}

#[tokio::test]
async fn security_t27_rotation_failure_rolls_back_consumption_and_replacement() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    seed_mcp(&db).await;
    let signer = SessionTestSigner::new();
    let session = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let manager = context(
        &session.claims,
        &session.access_token,
        lifecycle.clone(),
        clock.clone(),
    )
    .await;
    let old = issue(&lifecycle, &manager, &signer).await.unwrap();
    signer.fail.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        rotate(&lifecycle, &manager, old.metadata.id, &signer)
            .await
            .err(),
        Some(SecurityError::DependencyUnavailable)
    );
    signer
        .fail
        .store(false, std::sync::atomic::Ordering::SeqCst);
    execute(&db,"CREATE FUNCTION reject_rotation_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test commit rejected'; END $$").await.unwrap();
    execute(&db,"CREATE CONSTRAINT TRIGGER reject_rotation_commit AFTER INSERT ON mcp_token DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_rotation_commit()").await.unwrap();
    assert_eq!(
        rotate(&lifecycle, &manager, old.metadata.id, &signer)
            .await
            .err(),
        Some(SecurityError::DependencyUnavailable)
    );
    execute(&db, "DROP TRIGGER reject_rotation_commit ON mcp_token")
        .await
        .unwrap();
    let slow = SessionTestSigner {
        delay: std::time::Duration::from_millis(300),
        ..SessionTestSigner::new()
    };
    let budget = manager
        .budget()
        .child(std::time::Duration::from_millis(100))
        .unwrap();
    assert_eq!(
        lifecycle
            .rotate_mcp(
                &manager,
                McpRotationRequest {
                    id: old.metadata.id,
                    ttl_days: None
                },
                &config(),
                &slow,
                &budget
            )
            .await
            .err(),
        Some(SecurityError::DeadlineExceeded)
    );
    active(&lifecycle, &old, &clock).await;
    assert_eq!(session_count(&db,"security_credential WHERE token_use='mcp_access' AND (consumed_at IS NOT NULL OR replacement_jti IS NOT NULL OR revoked_at IS NOT NULL)").await,0);
    assert_eq!(session_count(&db, "mcp_token").await, 1);
    let new = rotate(&lifecycle, &manager, old.metadata.id, &signer)
        .await
        .unwrap();
    active(&lifecycle, &new, &clock).await;
}

#[tokio::test]
async fn security_t26_overlapping_deny_and_invalid_metadata_never_expand_ceilings() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    seed_mcp(&db).await;
    let broad = serde_json::to_string(
        &ResourceSelector::new(
            "resource-app",
            ResourceScope::Application {
                kind: ResourceKind::Device,
            },
        )
        .unwrap(),
    )
    .unwrap();
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO permission (application_id,name,action,resource_type,resource_pattern,create_time,update_time) SELECT id,'wide-read','device:read','device',$1,NOW(),NOW() FROM application WHERE app_key='resource-app'",[broad.into()])).await.unwrap();
    execute(&db,"INSERT INTO user_permission (user_id,permission_id,allow,create_time,update_time) SELECT 1,id,FALSE,NOW(),NOW() FROM permission WHERE name='t26-device'").await.unwrap();
    let signer = SessionTestSigner::new();
    let session = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let manager = context(
        &session.claims,
        &session.access_token,
        lifecycle.clone(),
        clock.clone(),
    )
    .await;
    let set = issue(&lifecycle, &manager, &signer).await.unwrap();
    assert!(
        set.claims
            .bounds
            .grants()
            .iter()
            .all(|g| g.operation().as_str() != "device:read")
    );
    execute(&db, "DELETE FROM user_permission WHERE user_id=1")
        .await
        .unwrap();
    let set = issue(&lifecycle, &manager, &signer).await.unwrap();
    assert!(
        set.claims
            .bounds
            .grants()
            .iter()
            .all(|g| g.operation().as_str() != "device:read")
    );
    execute(
        &db,
        "UPDATE permission SET resource_pattern='*' WHERE name='mcp:access'",
    )
    .await
    .unwrap();
    assert_eq!(
        issue(&lifecycle, &manager, &signer).await.err(),
        Some(SecurityError::Forbidden)
    );
    execute(&db,"UPDATE permission SET resource_type='device',resource_pattern='{}' WHERE name='mcp:access'").await.unwrap();
    assert_eq!(
        issue(&lifecycle, &manager, &signer).await.err(),
        Some(SecurityError::Forbidden)
    );
    let selector = serde_json::to_string(
        &ResourceSelector::new(
            "resource-app",
            ResourceScope::Application {
                kind: ResourceKind::Application,
            },
        )
        .unwrap(),
    )
    .unwrap();
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"UPDATE permission SET action='mcp:access',resource_type='application',resource_pattern=$1 WHERE name='mcp:access'",[selector.into()])).await.unwrap();
    execute(&db,"INSERT INTO permission (application_id,name,action,resource_type,resource_pattern,create_time,update_time) SELECT id,'ambiguous-deny','','application','*',NOW(),NOW() FROM application WHERE app_key='resource-app'").await.unwrap();
    execute(&db,"INSERT INTO user_permission (user_id,permission_id,allow,create_time,update_time) SELECT 1,id,FALSE,NOW(),NOW() FROM permission WHERE name='ambiguous-deny'").await.unwrap();
    assert_eq!(
        issue(&lifecycle, &manager, &signer).await.err(),
        Some(SecurityError::Forbidden)
    );
    assert_eq!(session_count(&db, "mcp_token").await, 2);
}

struct McpGateSigner {
    inner: SessionTestSigner,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
#[sea_orm::prelude::async_trait::async_trait]
impl SessionTokenSigner for McpGateSigner {
    fn key_id(&self) -> &str {
        self.inner.key_id()
    }
    async fn sign_access(
        &self,
        claims: &auth::security::credential::SecurityAccessClaims,
    ) -> Result<auth::security::identity::SecretToken, SecurityError> {
        self.entered.notify_one();
        self.release.notified().await;
        self.inner.sign_access(claims).await
    }
    async fn sign_id(
        &self,
        _claims: &auth::security::credential::SecurityAccessClaims,
        _client_id: &str,
        _nonce: Option<&str>,
        _access: &auth::security::identity::SecretToken,
    ) -> Result<auth::security::identity::SecretToken, SecurityError> {
        Err(SecurityError::Forbidden)
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn security_t26_application_revocation_blocks_mcp_from_an_older_snapshot() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    seed_mcp(&db).await;
    let signer = SessionTestSigner::new();
    let first = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let second = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let mint_manager = Arc::new(
        context(
            &first.claims,
            &first.access_token,
            lifecycle.clone(),
            clock.clone(),
        )
        .await,
    );
    let revoke_manager = context(
        &second.claims,
        &second.access_token,
        lifecycle.clone(),
        clock.clone(),
    )
    .await;
    let gate = Arc::new(McpGateSigner {
        inner: SessionTestSigner::new(),
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let job = {
        let (lifecycle, manager, gate) = (lifecycle.clone(), mint_manager.clone(), gate.clone());
        tokio::spawn(async move { issue(&lifecycle, &manager, gate.as_ref()).await })
    };
    tokio::time::timeout(std::time::Duration::from_secs(1), gate.entered.notified())
        .await
        .unwrap();
    lifecycle
        .revoke_credentials(
            &revoke_manager,
            RevocationScope::ApplicationGrants {
                application: "resource-app".into(),
            },
            &security_budget(&clock),
        )
        .await
        .unwrap();
    gate.release.notify_one();
    assert_eq!(job.await.unwrap().err(), Some(SecurityError::Conflict));
    assert_eq!(session_count(&db, "mcp_token").await, 0);
    assert_eq!(
        session_count(&db, "security_grant WHERE grant_kind='mcp'").await,
        0
    );
}

#[tokio::test]
async fn security_t27_current_permissions_and_management_uri_limit_rotation() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    seed_mcp(&db).await;
    let signer = SessionTestSigner::new();
    let session = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let manager = context(
        &session.claims,
        &session.access_token,
        lifecycle.clone(),
        clock.clone(),
    )
    .await;
    let old = issue(&lifecycle, &manager, &signer).await.unwrap();
    execute(&db,"INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) SELECT 1,id,FALSE,NOW(),NOW() FROM authority WHERE method='POST' AND authority_uri='/oidc/mcp-tokens/{id}/refresh'").await.unwrap();
    assert_eq!(
        rotate(&lifecycle, &manager, old.metadata.id, &signer)
            .await
            .err(),
        Some(SecurityError::Forbidden)
    );
    active(&lifecycle, &old, &clock).await;
    execute(&db, "DELETE FROM user_authority WHERE user_id=1")
        .await
        .unwrap();
    execute(&db,"INSERT INTO user_permission (user_id,permission_id,allow,create_time,update_time) SELECT 1,id,FALSE,NOW(),NOW() FROM permission WHERE name='mcp:access'").await.unwrap();
    assert_eq!(
        rotate(&lifecycle, &manager, old.metadata.id, &signer)
            .await
            .err(),
        Some(SecurityError::Forbidden)
    );
    execute(&db, "DELETE FROM user_permission WHERE user_id=1")
        .await
        .unwrap();
    execute(&db,"INSERT INTO user_permission (user_id,permission_id,allow,create_time,update_time) SELECT 1,id,FALSE,NOW(),NOW() FROM permission WHERE name='t26-device'").await.unwrap();
    let new = rotate(&lifecycle, &manager, old.metadata.id, &signer)
        .await
        .unwrap();
    assert!(
        new.claims
            .bounds
            .grants()
            .iter()
            .all(|g| g.operation().as_str() != "device:read")
    );
    active(&lifecycle, &new, &clock).await;
}
