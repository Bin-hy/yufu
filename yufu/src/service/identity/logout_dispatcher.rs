//! Short committed leases precede every external notification.
use super::{
    repository::PgIdentityRepository,
    repository_outbox::{DeliveryOutcome, LeasedLogout},
};
use crate::{
    config::Config, domain::identity::error::IdentityError, service::security::repository::now,
};
use auth::{
    claims::LogoutTokenClaims,
    security::{
        budget::{Clock, SecurityBudget},
        identity::SecretToken,
    },
    token_manager::IdTokenSigner,
};
use futures::{StreamExt, stream};
use std::sync::Arc;
use uuid::Uuid;

#[async_trait::async_trait]
#[allow(clippy::double_must_use)]
pub trait LogoutTransport: Send + Sync {
    async fn send(&self, endpoint: &str, token: &SecretToken) -> DeliveryOutcome;
}

pub struct LogoutDispatcher {
    repository: Arc<PgIdentityRepository>,
    clock: Arc<dyn Clock>,
    signer: Arc<IdTokenSigner>,
    config: Config,
    transport: Arc<dyn LogoutTransport>,
    worker: Uuid,
}
impl LogoutDispatcher {
    pub fn new(
        repository: Arc<PgIdentityRepository>,
        clock: Arc<dyn Clock>,
        signer: Arc<IdTokenSigner>,
        config: Config,
        transport: Arc<dyn LogoutTransport>,
    ) -> Result<Self, IdentityError> {
        config
            .validate_identity_policy()
            .map_err(|_| IdentityError::InvalidInput)?;
        Ok(Self {
            repository,
            clock,
            signer,
            config,
            transport,
            worker: Uuid::new_v4(),
        })
    }
    pub async fn dispatch_batch(&self) -> Result<usize, IdentityError> {
        // At most three waves of four deliveries, each bounded to two seconds.
        // Completion transactions have their own original two second budget.
        let leased = self
            .repository
            .lease_logout_with_policy(
                &SecurityBudget::new(self.clock.clone())?,
                self.worker,
                10,
                self.config.identity.logout_max_age_seconds,
            )
            .await?;
        let count = leased.len();
        let mut work =
            stream::iter(leased.into_iter().map(|row| self.dispatch_one(row))).buffer_unordered(4);
        let mut error = None;
        while let Some(result) = work.next().await {
            if let Err(reason) = result {
                error.get_or_insert(reason);
            }
        }
        if let Some(error) = error {
            return Err(error);
        }
        Ok(count)
    }
    async fn dispatch_one(&self, row: LeasedLogout) -> Result<(), IdentityError> {
        let outcome = match now(self.clock.as_ref()) {
            Err(_) => DeliveryOutcome::Retry {
                error_code: "clock_unavailable",
            },
            Ok(at) if row.issuer != self.config.issuer() || row.created_at > at => {
                DeliveryOutcome::PermanentFailure {
                    error_code: "invalid_endpoint",
                }
            }
            Ok(at) => {
                let claims = LogoutTokenClaims {
                    iss: row.issuer,
                    aud: row.client_id,
                    sub: row.subject,
                    sid: row.protocol_sid.to_string(),
                    iat: at / 1000,
                    exp: at / 1000 + 120,
                    jti: row.token_jti.to_string(),
                    events: std::collections::BTreeMap::from([(
                        "http://schemas.openid.net/event/backchannel-logout".into(),
                        serde_json::json!({}),
                    )]),
                };
                match self.signer.sign_logout_token(&claims) {
                    Ok(token) => match tokio::time::timeout(
                        std::time::Duration::from_millis(
                            self.config.identity.logout_timeout_millis,
                        ),
                        self.transport.send(&row.endpoint, &token),
                    )
                    .await
                    {
                        Ok(outcome) => outcome,
                        Err(_) => DeliveryOutcome::Retry {
                            error_code: "timeout",
                        },
                    },
                    Err(_) => DeliveryOutcome::Retry {
                        error_code: "signing_failed",
                    },
                }
            }
        };
        self.repository
            .finish_logout_with_policy(
                &SecurityBudget::new(self.clock.clone())?,
                row.delivery_id,
                self.worker,
                outcome,
                &self.config.identity.logout_retry_seconds,
                self.config.identity.logout_max_age_seconds,
            )
            .await
    }
}
