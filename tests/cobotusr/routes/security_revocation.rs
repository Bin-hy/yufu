use super::*;
use auth::security::{
    credential::{
        AuthenticatedContext, CredentialExpectation, CredentialProfile, CredentialRef,
        StrictAccessState,
    },
    error::SecurityError,
    identity::{PrincipalKind, SecretToken, TokenUse},
};
use axum::http;
use cobotusr::service::security::{
    lifecycle::{CredentialLifecycle, SessionTokenSet},
    revocation::{OidcRevocationRequest, RevocationScope},
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use std::sync::Arc;

async fn context_for(
    set: &SessionTokenSet,
    lifecycle: Arc<CredentialLifecycle>,
    clock: Arc<crate::cobotusr::common::SecurityTestClock>,
) -> AuthenticatedContext {
    let (jwks, _stop) = security_jwks().await;
    let expected = CredentialExpectation::new(
        &set.claims.iss,
        &set.claims.aud,
        &set.claims.application,
        vec![CredentialProfile::new(PrincipalKind::User, TokenUse::UserSession).unwrap()],
    )
    .unwrap();
    StrictAccessState::new(jwks, expected, clock, lifecycle)
        .authenticate(&http::HeaderMap::from_iter([(
            http::header::AUTHORIZATION,
            format!("Bearer {}", set.access_token.expose_secret())
                .parse()
                .unwrap(),
        )]))
        .await
        .unwrap()
}

async fn independent_mcp(
    db: &DatabaseConnection,
    set: &SessionTokenSet,
    jti: &str,
    application: &str,
) -> CredentialRef {
    use cobotusr::entity::{security_credential as credential, security_grant as grant};
    let row = credential::Entity::find_by_id((set.claims.iss.clone(), set.claims.jti.clone()))
        .one(db)
        .await
        .unwrap()
        .unwrap();
    let root = uuid::Uuid::new_v4();
    grant::ActiveModel {
        id: Set(root),
        issuer: Set(row.issuer.clone()),
        principal_kind: Set("user".into()),
        subject: Set(row.subject.clone()),
        application: Set(application.into()),
        grant_kind: Set("mcp".into()),
        created_at: Set(row.issued_at),
        revoked_at: Set(None),
    }
    .insert(db)
    .await
    .unwrap();
    let mut token: credential::ActiveModel = row.into();
    token.jti = Set(jti.into());
    token.grant_id = Set(root);
    token.token_use = Set("mcp_access".into());
    token.audience = Set(application.into());
    token.application = Set(application.into());
    token.session_id = Set(None);
    token.family_id = Set(None);
    token.oidc_client_id = Set(None);
    token.insert(db).await.unwrap();
    CredentialRef::new(&set.claims.iss, jti).unwrap()
}
async fn assert_active(
    lifecycle: &CredentialLifecycle,
    reference: &CredentialRef,
    clock: &Arc<crate::cobotusr::common::SecurityTestClock>,
) {
    lifecycle
        .check_registered(reference, &security_budget(clock))
        .await
        .unwrap();
}
async fn assert_revoked(
    lifecycle: &CredentialLifecycle,
    reference: &CredentialRef,
    clock: &Arc<crate::cobotusr::common::SecurityTestClock>,
) {
    assert_eq!(
        lifecycle
            .check_registered(reference, &security_budget(clock))
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
}

#[tokio::test]
async fn security_t25_logout_revokes_current_session_and_preserves_independent_roots() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    let signer = SessionTestSigner::new();
    let first = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let other = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let mcp = independent_mcp(&db, &first, "independent-mcp", "cobotusr").await;
    let original_ref = CredentialRef::new(&first.claims.iss, &first.claims.jti).unwrap();
    let rotated = lifecycle
        .rotate_local_refresh(
            &first.refresh_token,
            &session_config(),
            &signer,
            &security_budget(&clock),
        )
        .await
        .unwrap();
    let receipt = lifecycle
        .logout_local(
            &first.refresh_token,
            &first.claims.iss,
            None,
            &security_budget(&clock),
        )
        .await
        .unwrap();
    assert_eq!(receipt.effective_by - receipt.committed_at, 10_000);
    assert_revoked(
        &lifecycle,
        &CredentialRef::new(&rotated.claims.iss, &rotated.claims.jti).unwrap(),
        &clock,
    )
    .await;
    assert_revoked(&lifecycle, &original_ref, &clock).await;
    assert_eq!(
        lifecycle
            .rotate_local_refresh(
                &rotated.refresh_token,
                &session_config(),
                &signer,
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
    assert_active(
        &lifecycle,
        &CredentialRef::new(&other.claims.iss, &other.claims.jti).unwrap(),
        &clock,
    )
    .await;
    assert_active(&lifecycle, &mcp, &clock).await;
    lifecycle
        .logout_local(
            &first.refresh_token,
            &first.claims.iss,
            None,
            &security_budget(&clock),
        )
        .await
        .unwrap();
    assert_eq!(
        session_count(&db, "security_session WHERE revoked_at IS NOT NULL").await,
        1
    );
    assert_eq!(
        session_count(&db, "security_grant WHERE revoked_at IS NOT NULL").await,
        0
    );
}

#[tokio::test]
async fn security_t25_credential_and_family_scopes_follow_only_explicit_delegation() {
    use cobotusr::entity::security_credential as credential;
    let (_container, db, clock, lifecycle) = local_fixture().await;
    let signer = SessionTestSigner::new();
    let manager = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let parent = independent_mcp(&db, &manager, "mcp-parent", "resource-app").await;
    let unrelated = independent_mcp(&db, &manager, "mcp-audit-only", "resource-app").await;
    let row = credential::Entity::find_by_id((parent.issuer().to_owned(), parent.jti().to_owned()))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut child: credential::ActiveModel = row.into();
    child.jti = Set("mcp-delegation".into());
    child.token_use = Set("gateway_operation".into());
    child.audience = Set("legacy-gateway:g1".into());
    child.binding = Set(serde_json::json!({"kind":"gateway","gateway_id":"g1"}));
    child.revocation_parent_jti = Set(Some(parent.jti().into()));
    child.insert(&db).await.unwrap();
    execute(&db, "UPDATE security_credential SET minted_by='{\"issuer\":\"https://idp.test\",\"jti\":\"mcp-parent\"}' WHERE jti='mcp-audit-only'").await.unwrap();
    let context = context_for(&manager, lifecycle.clone(), clock.clone()).await;
    lifecycle
        .revoke_credentials(
            &context,
            RevocationScope::Credential(parent.clone()),
            context.budget(),
        )
        .await
        .unwrap();
    assert_revoked(&lifecycle, &parent, &clock).await;
    assert_revoked(
        &lifecycle,
        &CredentialRef::new(parent.issuer(), "mcp-delegation").unwrap(),
        &clock,
    )
    .await;
    assert_active(&lifecycle, &unrelated, &clock).await;
    let context = context_for(&manager, lifecycle.clone(), clock.clone()).await;
    lifecycle
        .revoke_credentials(&context, RevocationScope::RefreshFamily, context.budget())
        .await
        .unwrap();
    assert_eq!(
        session_count(&db, "security_session WHERE revoked_at IS NOT NULL").await,
        0
    );
    assert_eq!(
        session_count(&db, "security_refresh_family WHERE revoked_at IS NOT NULL").await,
        1
    );
}

#[tokio::test]
async fn security_t25_application_revocation_includes_mcp_and_all_matching_sessions() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    let signer = SessionTestSigner::new();
    let manager = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let first_code = session_code(&db, "session-test").await;
    let first = redeem_session(&lifecycle, &first_code, &signer, &security_budget(&clock))
        .await
        .unwrap();
    let second_code = session_code(&db, "session-test").await;
    let second = redeem_session(&lifecycle, &second_code, &signer, &security_budget(&clock))
        .await
        .unwrap();
    let mcp = independent_mcp(&db, &first, "app-mcp", "resource-app").await;
    let other = independent_mcp(&db, &manager, "other-app-mcp", "cobotusr").await;
    let context = context_for(&manager, lifecycle.clone(), clock.clone()).await;
    lifecycle
        .revoke_credentials(
            &context,
            RevocationScope::ApplicationGrants {
                application: "resource-app".into(),
            },
            context.budget(),
        )
        .await
        .unwrap();
    for reference in [
        mcp,
        CredentialRef::new(&first.claims.iss, &first.claims.jti).unwrap(),
        CredentialRef::new(&second.claims.iss, &second.claims.jti).unwrap(),
    ] {
        assert_revoked(&lifecycle, &reference, &clock).await;
    }
    assert_active(&lifecycle, &other, &clock).await;
    assert_active(
        &lifecycle,
        &CredentialRef::new(&manager.claims.iss, &manager.claims.jti).unwrap(),
        &clock,
    )
    .await;
    assert_eq!(
        session_count(&db, "security_grant WHERE revoked_at IS NOT NULL").await,
        3
    );
}

#[tokio::test]
async fn security_t25_commit_failure_and_inherited_lock_budget_do_not_report_revocation() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    let first = login_local_session(
        &lifecycle,
        &SessionTestSigner::new(),
        &security_budget(&clock),
    )
    .await
    .unwrap()
    .tokens;
    execute(&db,"CREATE FUNCTION reject_revoke_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test revoke rejected'; END $$").await.unwrap();
    execute(&db,"CREATE CONSTRAINT TRIGGER reject_revoke_commit AFTER UPDATE ON security_session DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_revoke_commit()").await.unwrap();
    assert_eq!(
        lifecycle
            .logout_local(
                &first.refresh_token,
                &first.claims.iss,
                None,
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::DependencyUnavailable)
    );
    assert_active(
        &lifecycle,
        &CredentialRef::new(&first.claims.iss, &first.claims.jti).unwrap(),
        &clock,
    )
    .await;
    assert_eq!(
        session_count(&db, "security_refresh_family WHERE revoked_at IS NOT NULL").await,
        0
    );
    execute(&db, "DROP TRIGGER reject_revoke_commit ON security_session")
        .await
        .unwrap();
    use sea_orm::TransactionTrait;
    let lock = db.begin().await.unwrap();
    lock.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id FROM security_grant WHERE id=$1 FOR UPDATE",
        [uuid::Uuid::parse_str(&first.claims.grant_id)
            .unwrap()
            .into()],
    ))
    .await
    .unwrap();
    let budget = security_budget(&clock)
        .child(std::time::Duration::from_millis(100))
        .unwrap();
    let started = std::time::Instant::now();
    assert!(matches!(
        lifecycle
            .logout_local(&first.refresh_token, &first.claims.iss, None, &budget)
            .await,
        Err(SecurityError::DeadlineExceeded | SecurityError::DependencyUnavailable)
    ));
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
    lock.rollback().await.unwrap();
    assert_active(
        &lifecycle,
        &CredentialRef::new(&first.claims.iss, &first.claims.jti).unwrap(),
        &clock,
    )
    .await;
    lifecycle
        .logout_local(
            &first.refresh_token,
            &first.claims.iss,
            None,
            &security_budget(&clock),
        )
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn security_t25_refresh_racing_logout_across_instances_cannot_restore_session() {
    use cobotusr::service::security::repository::PgSecurityRepository;
    let (_container, db, clock, lifecycle) = local_fixture().await;
    let signer = Arc::new(SessionTestSigner::new());
    let first = login_local_session(&lifecycle, signer.as_ref(), &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let second = Arc::new(CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
    ));
    let refresh = Arc::new(SecretToken::new(first.refresh_token.expose_secret().into()).unwrap());
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let job = tokio::spawn({
        let (barrier, refresh, clock, second) = (
            barrier.clone(),
            refresh.clone(),
            clock.clone(),
            second.clone(),
        );
        async move {
            barrier.wait().await;
            second
                .rotate_local_refresh(
                    &refresh,
                    &session_config(),
                    signer.as_ref(),
                    &security_budget(&clock),
                )
                .await
        }
    });
    barrier.wait().await;
    lifecycle
        .logout_local(&refresh, &first.claims.iss, None, &security_budget(&clock))
        .await
        .unwrap();
    match job.await.unwrap() {
        Ok(set) => {
            assert_revoked(
                &lifecycle,
                &CredentialRef::new(&set.claims.iss, &set.claims.jti).unwrap(),
                &clock,
            )
            .await
        }
        Err(error) => assert!(matches!(
            error,
            SecurityError::Conflict | SecurityError::CredentialRevoked
        )),
    }
    assert_eq!(
        session_count(&db, "security_session WHERE revoked_at IS NULL").await,
        0
    );
    assert_eq!(
        session_count(&db, "security_credential WHERE revoked_at IS NULL").await,
        0
    );
}

#[tokio::test]
async fn security_t25_oidc_revocation_authenticates_client_and_keeps_scope_isolated() {
    let (_container, _db, clock, lifecycle) = local_fixture().await;
    let local = login_local_session(
        &lifecycle,
        &SessionTestSigner::new(),
        &security_budget(&clock),
    )
    .await
    .unwrap()
    .tokens;
    assert_eq!(
        lifecycle
            .revoke_oidc(
                OidcRevocationRequest {
                    token: &local.refresh_token,
                    client_id: "unknown",
                    client_secret: None,
                    access_reference: None
                },
                &local.claims.iss,
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
    lifecycle
        .revoke_oidc(
            OidcRevocationRequest {
                token: &local.refresh_token,
                client_id: "session-test",
                client_secret: None,
                access_reference: None,
            },
            &local.claims.iss,
            &security_budget(&clock),
        )
        .await
        .unwrap();
    assert_active(
        &lifecycle,
        &CredentialRef::new(&local.claims.iss, &local.claims.jti).unwrap(),
        &clock,
    )
    .await;
    assert_eq!(
        lifecycle
            .logout_local(
                &local.refresh_token,
                "https://other-issuer.test",
                None,
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
}

#[tokio::test]
async fn security_t25_managed_session_and_kickout_preserve_mcp_and_enforce_owner_and_deny() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    let signer = SessionTestSigner::new();
    let manager = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let other = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let mcp = independent_mcp(&db, &manager, "kickout-independent", "cobotusr").await;
    let outsider = register_local_session(
        &lifecycle,
        "another-owner",
        &signer,
        &security_budget(&clock),
    )
    .await
    .unwrap()
    .tokens;
    let context = context_for(&manager, lifecycle.clone(), clock.clone()).await;
    let foreign = CredentialRef::new(&outsider.claims.iss, &outsider.claims.jti).unwrap();
    assert_eq!(
        lifecycle
            .revoke_credentials(
                &context,
                RevocationScope::Credential(foreign.clone()),
                context.budget()
            )
            .await
            .err(),
        Some(SecurityError::Forbidden)
    );
    assert_active(&lifecycle, &foreign, &clock).await;
    execute(&db, "INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) SELECT 1,id,FALSE,NOW(),NOW() FROM authority WHERE method='POST' AND authority_uri='/kickout'").await.unwrap();
    assert_eq!(
        lifecycle
            .kickout_sessions(&context, "1", context.budget())
            .await
            .err(),
        Some(SecurityError::Forbidden)
    );
    assert_active(
        &lifecycle,
        &CredentialRef::new(&other.claims.iss, &other.claims.jti).unwrap(),
        &clock,
    )
    .await;
    execute(&db, "DELETE FROM user_authority WHERE user_id=1 AND authority_id IN (SELECT id FROM authority WHERE method='POST' AND authority_uri='/kickout')").await.unwrap();
    lifecycle
        .revoke_credentials(&context, RevocationScope::CurrentSession, context.budget())
        .await
        .unwrap();
    assert_active(&lifecycle, &mcp, &clock).await;
    let context = context_for(&other, lifecycle.clone(), clock.clone()).await;
    lifecycle
        .kickout_sessions(&context, "1", context.budget())
        .await
        .unwrap();
    assert_active(&lifecycle, &mcp, &clock).await;
    assert_active(&lifecycle, &foreign, &clock).await;
    assert_eq!(
        session_count(
            &db,
            "security_session WHERE subject='1' AND revoked_at IS NULL"
        )
        .await,
        0
    );
}

#[tokio::test]
async fn security_t25_current_role_allow_cannot_expand_old_management_bounds() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    let session = register_local_session(
        &lifecycle,
        "bounded-manager",
        &SessionTestSigner::new(),
        &security_budget(&clock),
    )
    .await
    .unwrap()
    .tokens;
    let context = context_for(&session, lifecycle.clone(), clock.clone()).await;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET role_id=1 WHERE id=$1",
        [session.claims.sub.parse::<i32>().unwrap().into()],
    ))
    .await
    .unwrap();
    assert_eq!(
        lifecycle
            .kickout_sessions(&context, "1", context.budget())
            .await
            .err(),
        Some(SecurityError::Forbidden)
    );
    assert_active(
        &lifecycle,
        &CredentialRef::new(&session.claims.iss, &session.claims.jti).unwrap(),
        &clock,
    )
    .await;
}

struct PausedSigner {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    inner: SessionTestSigner,
}
#[sea_orm::prelude::async_trait::async_trait]
impl cobotusr::service::security::lifecycle::SessionTokenSigner for PausedSigner {
    fn key_id(&self) -> &str {
        "key"
    }
    async fn sign_access(
        &self,
        claims: &auth::security::credential::SecurityAccessClaims,
    ) -> Result<SecretToken, SecurityError> {
        self.entered.notify_one();
        self.release.notified().await;
        self.inner.sign_access(claims).await
    }
    async fn sign_id(
        &self,
        claims: &auth::security::credential::SecurityAccessClaims,
        client: &str,
        nonce: Option<&str>,
        access: &SecretToken,
    ) -> Result<SecretToken, SecurityError> {
        self.inner.sign_id(claims, client, nonce, access).await
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn security_t25_application_revocation_prevents_stale_transaction_minting_fresh_roots() {
    let (_container, db, clock, lifecycle) = local_fixture().await;
    let manager = login_local_session(
        &lifecycle,
        &SessionTestSigner::new(),
        &security_budget(&clock),
    )
    .await
    .unwrap()
    .tokens;
    let context = context_for(&manager, lifecycle.clone(), clock.clone()).await;
    let signer = Arc::new(PausedSigner {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
        inner: SessionTestSigner::new(),
    });
    let mint = tokio::spawn({
        let (lifecycle, signer, clock) = (lifecycle.clone(), signer.clone(), clock.clone());
        async move { login_local_session(&lifecycle, signer.as_ref(), &security_budget(&clock)).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(1), signer.entered.notified())
        .await
        .unwrap();
    lifecycle
        .revoke_credentials(
            &context,
            RevocationScope::ApplicationGrants {
                application: "cobotusr".into(),
            },
            context.budget(),
        )
        .await
        .unwrap();
    signer.release.notify_one();
    assert_eq!(mint.await.unwrap().err(), Some(SecurityError::Conflict));
    assert_eq!(
        session_count(&db, "security_grant WHERE revoked_at IS NULL").await,
        0
    );
    assert_eq!(
        session_count(&db, "security_credential WHERE revoked_at IS NULL").await,
        0
    );
}
