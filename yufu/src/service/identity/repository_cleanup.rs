//! Bounded cleanup preserves credential replay evidence and referenced tombstones.
use super::repository::{PgIdentityRepository, pg::signed};
use crate::{
    config_identity::Identity,
    domain::identity::error::IdentityError,
    service::security::repository::{SecurityTransaction, now},
};
use auth::security::budget::SecurityBudget;

impl PgIdentityRepository {
    pub async fn cleanup_identity(
        &self,
        budget: &SecurityBudget,
        policy: &Identity,
        limit: u32,
    ) -> Result<u64, IdentityError> {
        if limit == 0
            || limit > 1000
            || !(600..=365 * 86_400).contains(&policy.operation_retention_seconds)
            || !(86_400..=3650 * 86_400).contains(&policy.audit_retention_seconds)
        {
            return Err(IdentityError::InvalidInput);
        }
        let tx = self.security.begin_shared(budget).await?;
        let at = signed(now(tx.clock.as_ref())?)?;
        let audit_before = at.saturating_sub(signed(policy.audit_retention_seconds * 1000)?);
        let operation_before =
            at.saturating_sub(signed(policy.operation_retention_seconds * 1000)?);
        // Tables and predicates are closed source constants, never caller SQL.
        let commands = [
            (
                "identity_action_flow",
                "expires_at<=$1 AND (consumed_at IS NULL OR created_at<$2)",
            ),
            (
                "identity_authorization_flow",
                "expires_at<=$1 AND (completed_at IS NULL OR created_at<$2) AND NOT EXISTS (SELECT 1 FROM oauth_authorization_code c WHERE c.flow_id=t.id)",
            ),
            (
                "identity_invitation",
                "expires_at<$2 AND NOT EXISTS (SELECT 1 FROM identity_action_flow f WHERE f.invitation_id=t.id)",
            ),
            (
                "identity_password_reset",
                "expires_at<$2 AND NOT EXISTS (SELECT 1 FROM identity_action_flow f WHERE f.password_reset_id=t.id)",
            ),
            ("identity_request_operation", "committed_at<$3"),
            (
                "identity_logout_delivery",
                "created_at<$2 AND status IN ('delivered','permanent_failure','expired')",
            ),
            (
                "identity_audit_event",
                "happened_at<$2 AND NOT EXISTS (SELECT 1 FROM identity_logout_delivery d WHERE d.event_id=t.id) AND NOT EXISTS (SELECT 1 FROM identity_request_operation o WHERE o.id=t.operation_id)",
            ),
        ];
        let mut deleted = 0;
        for (table, predicate) in commands {
            // The values CTE gives all predicates the same typed cutoff parameters.
            let sql = format!(
                "WITH cutoffs AS (SELECT $1::bigint AS at,$2::bigint AS audit_before,$3::bigint AS operation_before), due AS (SELECT t.id FROM {table} t, cutoffs WHERE {predicate} ORDER BY t.id FOR UPDATE OF t SKIP LOCKED LIMIT $4) DELETE FROM {table} t USING due WHERE t.id=due.id"
            );
            deleted += tx
                .identity_execute(
                    &sql,
                    vec![
                        at.into(),
                        audit_before.into(),
                        operation_before.into(),
                        i64::from(limit).into(),
                    ],
                )
                .await?;
        }
        let stale_binding = at.saturating_sub(600_000);
        deleted += tx.identity_execute("WITH due AS (SELECT b.secret_digest FROM identity_browser_binding b WHERE b.touched_at<$1 AND NOT EXISTS (SELECT 1 FROM identity_action_flow f WHERE f.browser_binding_digest=b.secret_digest AND f.consumed_at IS NULL AND f.expires_at>$2) AND NOT EXISTS (SELECT 1 FROM identity_authorization_flow f WHERE f.browser_binding_digest=b.secret_digest AND f.completed_at IS NULL AND f.expires_at>$2) ORDER BY b.secret_digest FOR UPDATE OF b SKIP LOCKED LIMIT $3) DELETE FROM identity_browser_binding b USING due WHERE b.secret_digest=due.secret_digest",vec![stale_binding.into(),at.into(),i64::from(limit).into()]).await?;
        Box::new(tx).commit().await?;
        Ok(deleted)
    }
}
