use super::repository::{
    AuditEvent,
    pg::{get, unsigned},
};
use crate::{
    domain::identity::{
        audit::{AuditActor, AuditCursor, AuditFilter, AuditPage, AuditPosition},
        console::ConsoleAudit,
        error::IdentityError,
    },
    service::security::repository::PgSecurityTransaction,
};
use uuid::Uuid;

impl PgSecurityTransaction {
    pub(crate) async fn identity_audit_search(
        &self,
        filter: &AuditFilter,
        cursor: Option<&AuditCursor>,
        limit: u32,
    ) -> Result<AuditPage, IdentityError> {
        let (actor_mode, actor_id) = match filter.actor {
            None => (0, None),
            Some(AuditActor::Anonymous) => (1, None),
            Some(AuditActor::User(id)) => (2, Some(id)),
        };
        let rows=self.identity_rows(r#"SELECT e.id,e.request_id,e.actor_id,e.action,e.object_kind,e.object_id,e.happened_at,e.result,e.reason_code,e.change_summary
            FROM identity_audit_event e WHERE
            ($1::bigint IS NULL OR e.happened_at >= $1) AND ($2::bigint IS NULL OR e.happened_at < $2)
            AND ($3=0 OR ($3=1 AND e.actor_id IS NULL) OR ($3=2 AND e.actor_id=$4))
            AND ($5::text IS NULL OR e.action=$5) AND ($6::text IS NULL OR e.object_kind=$6)
            AND ($7::text IS NULL OR e.object_id=$7) AND ($8::uuid IS NULL OR e.request_id=$8)
            AND ($9::integer IS NULL OR EXISTS (SELECT 1 FROM identity_audit_application a WHERE a.event_id=e.id AND a.application_id=$9))
            AND ($10::text IS NULL OR e.result=$10)
            AND ($11::bigint IS NULL OR (e.happened_at,e.id)<=($11,$12))
            AND ($13::bigint IS NULL OR (e.happened_at,e.id)<($13,$14))
            ORDER BY e.happened_at DESC,e.id DESC LIMIT $15"#,
            vec![filter.from.into(),filter.to.into(),actor_mode.into(),actor_id.into(),filter.action.clone().into(),filter.object_kind.clone().into(),filter.object_id.clone().into(),filter.request_id.into(),filter.application_id.into(),filter.result.clone().into(),cursor.map(|c|c.upper.happened_at).into(),cursor.map(|c|c.upper.id).into(),cursor.map(|c|c.last.happened_at).into(),cursor.map(|c|c.last.id).into(),i64::from(limit+1).into()]).await?;
        let more = rows.len() > limit as usize;
        let upper = cursor.map(|c| c.upper).or(rows
            .first()
            .map(|row| {
                Ok::<_, IdentityError>(AuditPosition {
                    happened_at: get(row, "happened_at")?,
                    id: get(row, "id")?,
                })
            })
            .transpose()?);
        let mut items = Vec::new();
        let mut last = None;
        for row in rows.into_iter().take(limit as usize) {
            let at = get(&row, "happened_at")?;
            let id = get(&row, "id")?;
            let change_summary = get(&row, "change_summary")?;
            super::repository_outbox::validate_summary(&change_summary)?;
            last = Some(AuditPosition {
                happened_at: at,
                id,
            });
            items.push(ConsoleAudit {
                id,
                request_id: get(&row, "request_id")?,
                actor_id: get(&row, "actor_id")?,
                action: get(&row, "action")?,
                object_kind: get(&row, "object_kind")?,
                object_id: get(&row, "object_id")?,
                happened_at: unsigned(at)? / 1000,
                result: get(&row, "result")?,
                reason_code: get(&row, "reason_code")?,
                change_summary,
            });
        }
        let next = if more {
            match (upper, last) {
                (Some(upper), Some(last)) => Some(AuditCursor::new(filter, upper, last)?.encode()?),
                _ => return Err(IdentityError::DependencyUnavailable),
            }
        } else {
            None
        };
        Ok(AuditPage { items, next })
    }

    pub(crate) async fn identity_associate_audit(
        &self,
        event_id: Uuid,
        application_id: i32,
        environment_id: Option<Uuid>,
    ) -> Result<(), IdentityError> {
        self.identity_execute("INSERT INTO identity_audit_application (event_id,application_id,environment_id) SELECT $1,a.id,$3 FROM application a WHERE a.id=$2 AND ($3::uuid IS NULL OR EXISTS (SELECT 1 FROM identity_app_environment e WHERE e.id=$3 AND e.application_id=a.id)) ON CONFLICT (event_id,application_id) DO NOTHING",vec![event_id.into(),application_id.into(),environment_id.into()]).await?;
        Ok(())
    }

    pub(crate) async fn identity_audit_relations(
        &self,
        event: &AuditEvent,
    ) -> Result<(), IdentityError> {
        let rows = match event.object_kind {
            "application" => {
                let Some(id) = event.object_id.parse::<i32>().ok().filter(|id| *id > 0) else {
                    return Ok(());
                };
                self.identity_rows("SELECT id AS application_id,NULL::uuid AS environment_id FROM application WHERE id=$1",vec![id.into()]).await?
            }
            "environment" | "approval" | "session" | "logout_delivery" | "oauth_client" => {
                // The object namespace selects the lookup. Numeric collisions in other namespaces are never interpreted as apps.
                self.identity_rows(r#"SELECT e.application_id,e.id AS environment_id FROM identity_app_environment e WHERE
                    ($1='environment' AND e.id::text=$2)
                    OR ($1='approval' AND EXISTS(SELECT 1 FROM identity_app_approval p WHERE p.environment_id=e.id AND p.id::text=$2))
                    OR ($1='session' AND EXISTS(SELECT 1 FROM identity_oidc_rp_session p WHERE p.environment_id=e.id AND (p.id::text=$2 OR p.sso_session_id::text=$2)))
                    OR ($1='logout_delivery' AND EXISTS(SELECT 1 FROM identity_logout_delivery d JOIN identity_oidc_rp_session p ON p.id=d.rp_session_id WHERE p.environment_id=e.id AND d.id::text=$2))
                    OR ($1='oauth_client' AND EXISTS(SELECT 1 FROM oauth_client c WHERE c.environment_id=e.id AND c.id::text=$2))"#,vec![event.object_kind.into(),event.object_id.clone().into()]).await?
            }
            _ => return Ok(()),
        };
        for row in rows {
            self.identity_associate_audit(
                event.id,
                get(&row, "application_id")?,
                get(&row, "environment_id")?,
            )
            .await?;
        }
        Ok(())
    }
}
