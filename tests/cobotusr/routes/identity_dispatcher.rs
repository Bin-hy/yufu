use super::{super::common::IdentityFixture, identity_support::*};
use auth::security::identity::SecretToken;
use cobotusr::domain::identity::session::IdentityRevocationScope;
use cobotusr::service::identity::{
    logout_dispatcher::{LogoutDispatcher, LogoutTransport},
    repository_outbox::DeliveryOutcome,
    session::SessionService,
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

struct ScriptedTransport {
    db: sea_orm::DatabaseConnection,
    outcome: DeliveryOutcome,
    sent: Mutex<Vec<String>>,
}
#[async_trait::async_trait]
impl LogoutTransport for ScriptedTransport {
    async fn send(&self, endpoint: &str, token: &SecretToken) -> DeliveryOutcome {
        use sea_orm::{ConnectionTrait, DbBackend, Statement};
        assert_eq!(endpoint, "https://rp.example/backchannel");
        let row = self.db.query_one_raw(Statement::from_string(DbBackend::Postgres,"SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery WHERE status='leased'".to_owned())).await.unwrap().unwrap();
        assert!(
            row.try_get::<i64>("", "value").unwrap() > 0,
            "lease must have committed before network work"
        );
        self.sent
            .lock()
            .unwrap()
            .push(token.expose_secret().to_owned());
        self.outcome.clone()
    }
}
async fn enqueue(f: &IdentityFixture, key: &str) {
    let a = app(f, key, true).await;
    let (root, secret) = center_secret(f, a.owner).await;
    identity_tokens(f, &a, &root, "openid").await;
    SessionService::new(Arc::new(repository(f)), f.clock.clone())
        .revoke_personal(
            &secret,
            IdentityRevocationScope::CurrentCenter {
                sso_session_id: root.id,
            },
            Uuid::new_v4(),
            &budget(f),
        )
        .await
        .unwrap();
}
#[tokio::test]
async fn identity_session_dispatcher_committed_lease_stable_jti_retry_after_and_delivery() {
    let f = IdentityFixture::start().await;
    enqueue(&f, "dispatch-retry").await;
    let transport = Arc::new(ScriptedTransport {
        db: f.db.clone(),
        outcome: DeliveryOutcome::RetryAfter {
            error_code: "http_429",
            retry_after_seconds: 60,
        },
        sent: Mutex::new(vec![]),
    });
    let dispatcher = LogoutDispatcher::new(
        Arc::new(repository(&f)),
        f.clock.clone(),
        f.signer.clone(),
        f.config.clone(),
        transport.clone(),
    )
    .unwrap();
    assert_eq!(dispatcher.dispatch_batch().await.unwrap(), 1);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery WHERE status='pending' AND last_error_code='http_429' AND next_attempt_at=created_at+60000").await,1);
    assert_eq!(dispatcher.dispatch_batch().await.unwrap(), 0);
    f.clock.advance(60_001);
    let delivered = Arc::new(ScriptedTransport {
        db: f.db.clone(),
        outcome: DeliveryOutcome::Delivered,
        sent: Mutex::new(vec![]),
    });
    let next = LogoutDispatcher::new(
        Arc::new(repository(&f)),
        f.clock.clone(),
        f.signer.clone(),
        f.config.clone(),
        delivered.clone(),
    )
    .unwrap();
    assert_eq!(next.dispatch_batch().await.unwrap(), 1);
    let decode = |token: &str| {
        use base64::Engine;
        serde_json::from_slice::<serde_json::Value>(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(token.split('.').nth(1).unwrap())
                .unwrap(),
        )
        .unwrap()
    };
    let before = decode(&transport.sent.lock().unwrap()[0]);
    let after = decode(&delivered.sent.lock().unwrap()[0]);
    assert_eq!(before["jti"], after["jti"]);
    assert_eq!(before["sid"], after["sid"]);
    assert!(after.get("nonce").is_none());
    assert!(after["iat"].as_u64().unwrap() > before["iat"].as_u64().unwrap());
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery WHERE status='delivered' AND attempt_count=2").await,1);
    f.close().await;
}
#[tokio::test]
async fn identity_session_dispatcher_network_failure_preserves_revocation_and_permanent_failure_stops_retry()
 {
    let f = IdentityFixture::start().await;
    enqueue(&f, "dispatch-timeout").await;
    let transport = Arc::new(ScriptedTransport {
        db: f.db.clone(),
        outcome: DeliveryOutcome::Retry {
            error_code: "timeout",
        },
        sent: Mutex::new(vec![]),
    });
    let dispatcher = LogoutDispatcher::new(
        Arc::new(repository(&f)),
        f.clock.clone(),
        f.signer.clone(),
        f.config.clone(),
        transport,
    )
    .unwrap();
    assert_eq!(dispatcher.dispatch_batch().await.unwrap(), 1);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_sso_session WHERE revoked_at IS NULL"
        )
        .await,
        0
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_oidc_rp_session WHERE revoked_at IS NULL").await,0);
    f.clock.advance(5001);
    let failed = Arc::new(ScriptedTransport {
        db: f.db.clone(),
        outcome: DeliveryOutcome::PermanentFailure {
            error_code: "http_4xx",
        },
        sent: Mutex::new(vec![]),
    });
    let next = LogoutDispatcher::new(
        Arc::new(repository(&f)),
        f.clock.clone(),
        f.signer.clone(),
        f.config.clone(),
        failed,
    )
    .unwrap();
    assert_eq!(next.dispatch_batch().await.unwrap(), 1);
    f.clock.advance(3_600_001);
    assert_eq!(next.dispatch_batch().await.unwrap(), 0);
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery WHERE status='permanent_failure'").await,1);
    f.close().await;
}

struct HoldingTransport(tokio::sync::Notify);
#[async_trait::async_trait]
impl LogoutTransport for HoldingTransport {
    async fn send(&self, _endpoint: &str, _token: &SecretToken) -> DeliveryOutcome {
        self.0.notify_one();
        std::future::pending().await
    }
}
#[tokio::test]
async fn identity_session_workers_cancel_inflight_transport_and_restart_recovers_lease() {
    use cobotusr::service::identity::workers::IdentityWorkers;
    let f = IdentityFixture::start().await;
    enqueue(&f, "dispatch-cancel").await;
    let repo = Arc::new(repository(&f));
    let holding = Arc::new(HoldingTransport(tokio::sync::Notify::new()));
    let dispatcher = Arc::new(
        LogoutDispatcher::new(
            repo.clone(),
            f.clock.clone(),
            f.signer.clone(),
            f.config.clone(),
            holding.clone(),
        )
        .unwrap(),
    );
    let workers =
        IdentityWorkers::start(dispatcher, repo, f.clock.clone(), f.config.identity.clone());
    tokio::time::timeout(std::time::Duration::from_secs(2), holding.0.notified())
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), workers.shutdown())
        .await
        .unwrap();
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_logout_delivery WHERE status='leased'"
        )
        .await,
        1
    );
    f.clock.advance(30_001);
    let delivered = Arc::new(ScriptedTransport {
        db: f.db.clone(),
        outcome: DeliveryOutcome::Delivered,
        sent: Mutex::new(vec![]),
    });
    let restarted = LogoutDispatcher::new(
        Arc::new(repository(&f)),
        f.clock.clone(),
        f.signer.clone(),
        f.config.clone(),
        delivered,
    )
    .unwrap();
    assert_eq!(restarted.dispatch_batch().await.unwrap(), 1);
    f.close().await;
}
#[tokio::test]
async fn identity_session_cleanup_deletes_expired_actions_but_retains_replay_evidence_and_audit() {
    use cobotusr::{
        domain::identity::account::{IssuedSecret, SecretDigest},
        service::identity::{
            logout::{LogoutRequest, LogoutService},
            repository::*,
        },
    };
    let f = IdentityFixture::start().await;
    let a = app(&f, "cleanup", true).await;
    let (root, secret) = center_secret(&f, a.owner).await;
    identity_tokens(&f, &a, &root, "openid").await;
    let s = LogoutService::new(
        Arc::new(repository(&f)),
        f.clock.clone(),
        f.signer.clone(),
        f.config.clone(),
    );
    s.start(
        Some(&secret),
        SecretDigest::of(&IssuedSecret::from_entropy([45; 32]).into_response()),
        LogoutRequest {
            client_id: Some("cleanup-dev".into()),
            ..Default::default()
        },
        &budget(&f),
    )
    .await
    .unwrap();
    let credentials = scalar(
        &f,
        "SELECT COUNT(*)::bigint AS value FROM security_credential",
    )
    .await;
    let repo = repository(&f);
    let mut tx = repo.begin(&budget(&f)).await.unwrap();
    tx.apply(IdentityWrite::SaveOperation(OperationReceiptWrite {
        id: Uuid::new_v4(),
        actor_id: a.owner,
        operation_kind: "cleanup-test",
        idempotency_key: "cleanup".into(),
        request_hash: "a".repeat(64),
        resource_id: a.app.to_string(),
        summary: serde_json::json!({"application_id":a.app}),
    }))
    .await
    .unwrap();
    tx.commit().await.unwrap();
    f.clock.advance(600_001);
    assert!(
        repo.cleanup_identity(&budget(&f), &f.config.identity, 100)
            .await
            .unwrap()
            > 0
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM identity_action_flow"
        )
        .await,
        0
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM security_credential"
        )
        .await,
        credentials
    );
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_request_operation WHERE operation_kind='cleanup-test'").await,1);
    f.clock.advance(86_400_000);
    repo.cleanup_identity(&budget(&f), &f.config.identity, 100)
        .await
        .unwrap();
    assert_eq!(scalar(&f,"SELECT COUNT(*)::bigint AS value FROM identity_request_operation WHERE operation_kind='cleanup-test'").await,0);
    assert_eq!(
        scalar(
            &f,
            "SELECT COUNT(*)::bigint AS value FROM security_credential"
        )
        .await,
        credentials
    );
    f.close().await;
}
