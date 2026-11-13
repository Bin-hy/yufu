use super::repository::pg::{get, signed};
use super::repository::*;
use crate::domain::identity::error::IdentityError;
use crate::service::security::repository::{PgSecurityTransaction, SecurityTransaction, now};
use auth::security::budget::SecurityBudget;
use uuid::Uuid;

/// A leased row contains a stable JTI and approved endpoint, never a signed token.
pub struct LeasedLogout {
    pub delivery_id: Uuid,
    pub event_id: Uuid,
    pub rp_session_id: Uuid,
    pub client_id: String,
    pub endpoint: String,
    pub token_jti: Uuid,
    pub attempt_count: u32,
    pub created_at: u64,
    pub issuer: String,
    pub subject: String,
    pub protocol_sid: Uuid,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeliveryOutcome {
    Delivered,
    Retry {
        error_code: &'static str,
    },
    RetryAfter {
        error_code: &'static str,
        retry_after_seconds: u64,
    },
    PermanentFailure {
        error_code: &'static str,
    },
}

impl PgIdentityRepository {
    pub async fn lease_logout(
        &self,
        budget: &SecurityBudget,
        worker: Uuid,
        limit: u32,
    ) -> Result<Vec<LeasedLogout>, IdentityError> {
        self.lease_logout_with_policy(budget, worker, limit, 86_400)
            .await
    }
    pub async fn lease_logout_with_policy(
        &self,
        budget: &SecurityBudget,
        worker: Uuid,
        limit: u32,
        max_age_seconds: u64,
    ) -> Result<Vec<LeasedLogout>, IdentityError> {
        if limit == 0 || limit > 100 {
            return Err(IdentityError::InvalidInput);
        }
        if worker.is_nil() || !(1..=86_400).contains(&max_age_seconds) {
            return Err(IdentityError::InvalidInput);
        }
        let transaction = self.security.begin_shared(budget).await?;
        // Lease methods use the same executor as identity writes; external
        // delivery starts only after this short transaction has committed.
        let rows = transaction
            .identity_lease_logout_rows(worker, limit, max_age_seconds)
            .await?;
        Box::new(transaction).commit().await?;
        Ok(rows)
    }
    pub async fn finish_logout(
        &self,
        budget: &SecurityBudget,
        delivery: Uuid,
        worker: Uuid,
        outcome: DeliveryOutcome,
    ) -> Result<(), IdentityError> {
        self.finish_logout_with_policy(
            budget,
            delivery,
            worker,
            outcome,
            &[5, 30, 120, 600, 3600],
            86_400,
        )
        .await
    }
    pub async fn finish_logout_with_policy(
        &self,
        budget: &SecurityBudget,
        delivery: Uuid,
        worker: Uuid,
        outcome: DeliveryOutcome,
        retry_seconds: &[u64],
        max_age_seconds: u64,
    ) -> Result<(), IdentityError> {
        if retry_seconds.is_empty()
            || retry_seconds.len() > 16
            || !(1..=86_400).contains(&max_age_seconds)
            || retry_seconds
                .iter()
                .any(|delay| *delay == 0 || *delay > max_age_seconds)
            || retry_seconds.windows(2).any(|pair| pair[0] > pair[1])
        {
            return Err(IdentityError::InvalidInput);
        }
        let transaction = self.security.begin_shared(budget).await?;
        transaction
            .identity_finish_logout(delivery, worker, outcome, retry_seconds, max_age_seconds)
            .await?;
        Box::new(transaction).commit().await?;
        Ok(())
    }
}

impl PgSecurityTransaction {
    pub(crate) async fn write_identity_operation(
        &self,
        operation: OperationReceiptWrite,
    ) -> Result<IdentityWriteResult, IdentityError> {
        validate_summary(&operation.summary)?;
        let at = signed(now(self.clock.as_ref())?)?;
        self.identity_execute("INSERT INTO identity_request_operation (id,actor_id,operation_kind,idempotency_key,request_hash,resource_id,result_summary,committed_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",vec![operation.id.into(),operation.actor_id.into(),operation.operation_kind.into(),operation.idempotency_key.into(),operation.request_hash.into(),operation.resource_id.into(),operation.summary.into(),at.into()]).await?;
        Ok(IdentityWriteResult::Resource(operation.id))
    }
    pub(crate) async fn write_identity_audit(
        &self,
        event: &AuditEvent,
    ) -> Result<(), IdentityError> {
        validate_summary(&event.change_summary)?;
        if matches!(event.result, AuditResult::Success) && event.commit_marker.is_none() {
            return Err(IdentityError::InvalidInput);
        }
        let at = signed(now(self.clock.as_ref())?)?;
        self.identity_execute("INSERT INTO identity_audit_event (id,request_id,actor_id,action,object_kind,object_id,happened_at,result,reason_code,change_summary,operation_id,commit_marker) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",vec![event.id.into(),event.request_id.into(),event.actor_id.into(),event.action.into(),event.object_kind.into(),event.object_id.clone().into(),at.into(),match event.result { AuditResult::Success=>"success",AuditResult::Denied=>"denied",AuditResult::Failure=>"failure" }.into(),event.reason.map(|e|e.code()).into(),event.change_summary.clone().into(),event.operation_id.into(),event.commit_marker.into()]).await?;
        self.identity_audit_relations(event).await?;
        Ok(())
    }
    pub(crate) async fn write_identity_logout(
        &self,
        event: &LogoutEvent,
    ) -> Result<(), IdentityError> {
        let at = signed(now(self.clock.as_ref())?)?;
        for rp_id in &event.rp_session_ids {
            let row=self.identity_one("SELECT r.config_version_id,c.client_id FROM identity_oidc_rp_session r JOIN oauth_client c ON c.id=r.oauth_client_id WHERE r.id=$1 AND r.revoked_at IS NOT NULL",vec![(*rp_id).into()]).await?;
            let config = self
                .identity_version_config(get(&row, "config_version_id")?, true)
                .await?;
            if let Some(endpoint) = config.backchannel_logout_uri {
                self.identity_execute("INSERT INTO identity_logout_delivery (id,event_id,rp_session_id,client_id,endpoint_snapshot,next_attempt_at,token_jti,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$6) ON CONFLICT (event_id,rp_session_id) DO NOTHING",vec![Uuid::new_v4().into(),event.audit_event_id.into(),(*rp_id).into(),get::<String>(&row,"client_id")?.into(),endpoint.as_str().into(),at.into(),Uuid::new_v4().into()]).await?;
            }
        }
        Ok(())
    }
    pub(crate) async fn identity_lease_logout_rows(
        &self,
        worker: Uuid,
        limit: u32,
        max_age_seconds: u64,
    ) -> Result<Vec<LeasedLogout>, IdentityError> {
        let at = signed(now(self.clock.as_ref())?)?;
        let horizon = signed(max_age_seconds * 1000)?;
        self.identity_execute("WITH expired AS (SELECT id FROM identity_logout_delivery WHERE status IN ('pending','leased') AND created_at<=$1-$2 ORDER BY id FOR UPDATE SKIP LOCKED LIMIT 100) UPDATE identity_logout_delivery d SET status='expired',lease_owner=NULL,lease_until=NULL FROM expired WHERE d.id=expired.id",vec![at.into(),horizon.into()]).await?;
        let rows=self.identity_rows("WITH due AS (SELECT id FROM identity_logout_delivery WHERE created_at>$1-$4 AND ((status='pending' AND next_attempt_at<=$1) OR (status='leased' AND lease_until<=$1)) ORDER BY next_attempt_at,id FOR UPDATE SKIP LOCKED LIMIT $2), claimed AS (UPDATE identity_logout_delivery d SET status='leased',lease_owner=$3,lease_until=$1+30000,attempt_count=attempt_count+1 FROM due WHERE d.id=due.id RETURNING d.*) SELECT d.*,r.issuer,r.subject,r.protocol_sid FROM claimed d JOIN identity_oidc_rp_session r ON r.id=d.rp_session_id",vec![at.into(),i64::from(limit).into(),worker.into(),horizon.into()]).await?;
        rows.into_iter()
            .map(|row| {
                Ok(LeasedLogout {
                    delivery_id: get(&row, "id")?,
                    event_id: get(&row, "event_id")?,
                    rp_session_id: get(&row, "rp_session_id")?,
                    client_id: get(&row, "client_id")?,
                    endpoint: get(&row, "endpoint_snapshot")?,
                    token_jti: get(&row, "token_jti")?,
                    attempt_count: u32::try_from(get::<i32>(&row, "attempt_count")?)
                        .map_err(|_| IdentityError::DependencyUnavailable)?,
                    created_at: super::repository::pg::unsigned(get(&row, "created_at")?)?,
                    issuer: get(&row, "issuer")?,
                    subject: get(&row, "subject")?,
                    protocol_sid: get(&row, "protocol_sid")?,
                })
            })
            .collect()
    }
    pub(crate) async fn identity_finish_logout(
        &self,
        delivery: Uuid,
        worker: Uuid,
        outcome: DeliveryOutcome,
        retry_seconds: &[u64],
        max_age_seconds: u64,
    ) -> Result<(), IdentityError> {
        let at = signed(now(self.clock.as_ref())?)?;
        let (status, error, retry_after) = match outcome {
            DeliveryOutcome::Delivered => ("delivered", None, 0),
            DeliveryOutcome::Retry { error_code } => ("pending", Some(error_code), 0),
            DeliveryOutcome::RetryAfter {
                error_code,
                retry_after_seconds,
            } => (
                "pending",
                Some(error_code),
                retry_after_seconds.min(max_age_seconds),
            ),
            DeliveryOutcome::PermanentFailure { error_code } => {
                ("permanent_failure", Some(error_code), 0)
            }
        };
        if error.is_some_and(|e| {
            ![
                "timeout",
                "network_error",
                "http_5xx",
                "http_4xx",
                "http_429",
                "http_redirect",
                "invalid_endpoint",
                "clock_unavailable",
                "signing_failed",
                "ssrf_blocked",
            ]
            .contains(&e)
        }) {
            return Err(IdentityError::InvalidInput);
        }
        let row = self.identity_rows("SELECT attempt_count FROM identity_logout_delivery WHERE id=$1 AND status='leased' AND lease_owner=$2 AND lease_until>$3 FOR UPDATE",vec![delivery.into(),worker.into(),at.into()]).await?;
        let attempt = row
            .first()
            .ok_or(IdentityError::Conflict)
            .and_then(|row| get::<i32>(row, "attempt_count"))?;
        let index = usize::try_from(attempt.saturating_sub(1))
            .map_err(|_| IdentityError::DependencyUnavailable)?
            .min(retry_seconds.len() - 1);
        let delay = signed(retry_seconds[index].max(retry_after) * 1000)?;
        let horizon = signed(max_age_seconds * 1000)?;
        super::repository::pg::one_row(self.identity_execute("UPDATE identity_logout_delivery SET status=CASE WHEN created_at<=$3-$7 THEN 'expired' ELSE $4 END,last_error_code=$5,next_attempt_at=LEAST($3+$6,created_at+$7),delivered_at=CASE WHEN $4='delivered' AND created_at>$3-$7 THEN $3 ELSE NULL END,lease_owner=NULL,lease_until=NULL WHERE id=$1 AND status='leased' AND lease_owner=$2 AND lease_until>$3",vec![delivery.into(),worker.into(),at.into(),status.into(),error.into(),delay.into(),horizon.into()]).await?)
    }
}

/// Closed structural allowlist: receipts and audit summaries cannot become a
/// second storage channel for credentials or protocol payloads.
pub(crate) fn validate_summary(summary: &serde_json::Value) -> Result<(), IdentityError> {
    let map = summary.as_object().ok_or(IdentityError::InvalidInput)?;
    const KEYS: &[&str] = &[
        "application_id",
        "application_key",
        "environment_id",
        "environment_ids",
        "user_id",
        "group_id",
        "version_id",
        "previous_version_id",
        "owner_user_id",
        "previous_owner_user_id",
        "development_client_id",
        "production_client_id",
        "production_version_id",
        "client_id",
        "reader_resource_id",
        "reader_id",
        "client_kind",
        "management_mode",
        "enabled",
        "developer_enabled",
        "person_kind",
        "present",
        "active",
        "status",
        "revision",
        "secret_revision",
        "group_id",
        "metadata_changed",
        "generation",
        "auth_epoch",
        "lifecycle_epoch",
        "scope",
        "affected_count",
        "logout_capability",
        "logout_limitation_accepted",
        "invitation_id",
        "reset_id",
        "approval_id",
        "operation_id",
        "request_id",
        "member_role",
        "target_user_id",
        "expires_at",
        "created",
        "developer_grant",
        "revoked",
        "profile_changed",
    ];
    if map.len() > 32
        || map.iter().any(|(key, value)| {
            !KEYS.contains(&key.as_str())
                || match value {
                    serde_json::Value::Null
                    | serde_json::Value::Bool(_)
                    | serde_json::Value::Number(_) => false,
                    serde_json::Value::String(s) => {
                        s.len() > 256 || s.chars().any(char::is_control)
                    }
                    serde_json::Value::Array(a) => {
                        a.len() > 100 || a.iter().any(|v| v.as_str().is_none_or(|s| s.len() > 128))
                    }
                    serde_json::Value::Object(_) => true,
                }
        })
    {
        return Err(IdentityError::InvalidInput);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipts_and_audits_reject_protocol_and_credential_payloads() {
        for key in [
            "secret",
            "password",
            "code",
            "nonce",
            "state",
            "client_secret_hash",
            "token",
            "refresh_token",
            "authorization",
            "request_payload",
        ] {
            assert_eq!(
                validate_summary(&serde_json::json!({key:"hidden"})),
                Err(IdentityError::InvalidInput)
            );
        }
        assert!(
            validate_summary(&serde_json::json!({"application_id":7,"enabled":false,"revision":3}))
                .is_ok()
        );
        assert!(
            validate_summary(&serde_json::json!({"application_id":{"code":"nested"}})).is_err()
        );
    }
}
