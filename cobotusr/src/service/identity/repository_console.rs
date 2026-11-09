use super::repository::{
    AdmissionQuery, IdentityTransaction, IdentityWrite, IdentityWriteResult,
    pg::{get, signed, unsigned},
};
use crate::{
    domain::identity::{application::IdentityScopeSet, console::*, error::IdentityError},
    service::security::repository::{PgSecurityTransaction, now},
};
use uuid::Uuid;

impl PgSecurityTransaction {
    pub(crate) async fn identity_console_read(
        &mut self,
        query: ConsoleQuery,
    ) -> Result<ConsoleResult, IdentityError> {
        match query {
            ConsoleQuery::Accounts { after, limit } => {
                let rows=self.identity_rows("SELECT id,username,nickname,enabled,person_kind,developer_enabled,identity_revision FROM \"user\" WHERE id>$1 ORDER BY id LIMIT $2",vec![after.into(),i64::from(limit+1).into()]).await?;
                let items = rows
                    .into_iter()
                    .map(|r| {
                        Ok(ConsoleAccount {
                            user_id: get(&r, "id")?,
                            username: get(&r, "username")?,
                            nickname: get::<Option<String>>(&r, "nickname")?.unwrap_or_default(),
                            enabled: get(&r, "enabled")?,
                            person_kind: get(&r, "person_kind")?,
                            developer_enabled: get(&r, "developer_enabled")?,
                            revision: unsigned(get(&r, "identity_revision")?)?,
                        })
                    })
                    .collect::<Result<Vec<_>, IdentityError>>()?;
                Ok(ConsoleResult::Accounts(page(items, limit, |r| r.user_id)))
            }
            ConsoleQuery::Centers {
                user_id,
                current_id,
                after,
                limit,
            } => {
                let rows=self.identity_rows("SELECT id,authenticated_at,expires_at,revoked_at FROM identity_sso_session WHERE user_id=$1 AND ($2::uuid IS NULL OR id>$2) ORDER BY id LIMIT $3",vec![user_id.into(),after.into(),i64::from(limit+1).into()]).await?;
                let items = rows
                    .into_iter()
                    .map(|r| {
                        let id = get(&r, "id")?;
                        Ok(PersonalCenter {
                            id,
                            current: id == current_id,
                            auth_time: unsigned(get(&r, "authenticated_at")?)? / 1000,
                            expires_at: unsigned(get(&r, "expires_at")?)? / 1000,
                            revoked_at: get::<Option<i64>>(&r, "revoked_at")?
                                .map(unsigned)
                                .transpose()?
                                .map(|v| v / 1000),
                        })
                    })
                    .collect::<Result<Vec<_>, IdentityError>>()?;
                Ok(ConsoleResult::Centers(page(items, limit, |r| r.id)))
            }
            ConsoleQuery::RelyingParties {
                user_id,
                after,
                limit,
            } => {
                let rows=self.identity_rows("SELECT p.id,p.sso_session_id,p.created_at,p.revoked_at,r.expires_at,r.revoked_at AS center_revoked_at,a.name,e.kind FROM identity_oidc_rp_session p JOIN identity_sso_session r ON r.id=p.sso_session_id JOIN identity_app_environment e ON e.id=p.environment_id JOIN application a ON a.id=e.application_id WHERE r.user_id=$1 AND ($2::uuid IS NULL OR p.id>$2) ORDER BY p.id LIMIT $3",vec![user_id.into(),after.into(),i64::from(limit+1).into()]).await?;
                let items = rows
                    .into_iter()
                    .map(|r| {
                        Ok(PersonalRelyingParty {
                            id: get(&r, "id")?,
                            center_id: get(&r, "sso_session_id")?,
                            name: get(&r, "name")?,
                            environment: get(&r, "kind")?,
                            created_at: unsigned(get(&r, "created_at")?)? / 1000,
                            revoked_at: get::<Option<i64>>(&r, "revoked_at")?
                                .map(unsigned)
                                .transpose()?
                                .map(|v| v / 1000),
                            center_expires_at: unsigned(get(&r, "expires_at")?)? / 1000,
                            center_revoked: get::<Option<i64>>(&r, "center_revoked_at")?.is_some(),
                        })
                    })
                    .collect::<Result<Vec<_>, IdentityError>>()?;
                Ok(ConsoleResult::RelyingParties(page(items, limit, |r| r.id)))
            }
            ConsoleQuery::Consents {
                user_id,
                after,
                limit,
            } => {
                let rows=self.identity_rows("SELECT c.id,c.environment_id,c.config_version_id,c.scope,e.application_id,e.kind,e.active_version_id,a.name FROM oauth_consent c JOIN identity_app_environment e ON e.id=c.environment_id JOIN application a ON a.id=e.application_id WHERE c.user_id=$1 AND c.id>$2 ORDER BY c.id LIMIT $3",vec![user_id.into(),after.into(),i64::from(limit+1).into()]).await?;
                let items = rows
                    .into_iter()
                    .map(|r| {
                        let version_id = get(&r, "config_version_id")?;
                        Ok(PersonalConsent {
                            id: get(&r, "id")?,
                            application_id: get(&r, "application_id")?,
                            name: get(&r, "name")?,
                            environment_id: get(&r, "environment_id")?,
                            environment: get(&r, "kind")?,
                            version_id,
                            scopes: get::<String>(&r, "scope")?
                                .split_whitespace()
                                .map(str::to_owned)
                                .collect(),
                            current_version: get::<Option<Uuid>>(&r, "active_version_id")?
                                == Some(version_id),
                        })
                    })
                    .collect::<Result<Vec<_>, IdentityError>>()?;
                Ok(ConsoleResult::Consents(page(items, limit, |r| r.id)))
            }
            ConsoleQuery::Applications {
                user_id,
                root_id,
                after,
                limit,
            } => {
                // Select only currently assigned candidates, then run the same admission evaluator used by issuance.
                let rows=self.identity_rows("SELECT e.id,e.application_id,e.kind,a.name,c.client_id FROM identity_app_environment e JOIN application a ON a.id=e.application_id JOIN oauth_client c ON c.id=e.oauth_client_id WHERE a.management_state='managed' AND a.lifecycle='active' AND e.active_version_id IS NOT NULL AND ($2::uuid IS NULL OR e.id>$2) AND ((e.kind='production' AND EXISTS (SELECT 1 FROM identity_app_assignment x WHERE x.application_id=a.id AND (x.user_id=$1 OR EXISTS (SELECT 1 FROM identity_group_member gm WHERE gm.group_id=x.group_id AND gm.user_id=$1)))) OR (e.kind='development' AND (EXISTS (SELECT 1 FROM identity_app_member m WHERE m.application_id=a.id AND m.user_id=$1) OR EXISTS (SELECT 1 FROM identity_app_test_user t WHERE t.environment_id=e.id AND t.user_id=$1)))) ORDER BY e.id LIMIT $3",vec![user_id.into(),after.into(),i64::from(limit+1).into()]).await?;
                let mut candidates = rows
                    .into_iter()
                    .map(|r| {
                        Ok(PersonalApplication {
                            environment_id: get(&r, "id")?,
                            application_id: get(&r, "application_id")?,
                            name: get(&r, "name")?,
                            environment: get(&r, "kind")?,
                            client_id: get(&r, "client_id")?,
                        })
                    })
                    .collect::<Result<Vec<_>, IdentityError>>()?;
                let more = candidates.len() > limit as usize;
                candidates.truncate(limit as usize);
                let next = if more {
                    candidates.last().map(|c| c.environment_id)
                } else {
                    None
                };
                let mut items = Vec::new();
                for candidate in candidates {
                    match self
                        .evaluate_initial_admission(&AdmissionQuery {
                            sso_session_id: root_id,
                            environment_id: candidate.environment_id,
                            requested_scopes: IdentityScopeSet::parse("openid")?,
                        })
                        .await
                    {
                        Ok(_) => items.push(candidate),
                        Err(IdentityError::AccessDenied | IdentityError::Forbidden) => {}
                        Err(error) => return Err(error),
                    }
                }
                Ok(ConsoleResult::Applications(ConsolePage { items, next }))
            }
            ConsoleQuery::Audit { after, limit } => {
                let rows=self.identity_rows("SELECT id,request_id,actor_id,action,object_kind,object_id,happened_at,result,reason_code,change_summary FROM identity_audit_event WHERE ($1::uuid IS NULL OR id>$1) ORDER BY id LIMIT $2",vec![after.into(),i64::from(limit+1).into()]).await?;
                let items = rows
                    .into_iter()
                    .map(|r| {
                        let change_summary = get(&r, "change_summary")?;
                        super::repository_outbox::validate_summary(&change_summary)?;
                        Ok(ConsoleAudit {
                            id: get(&r, "id")?,
                            request_id: get(&r, "request_id")?,
                            actor_id: get(&r, "actor_id")?,
                            action: get(&r, "action")?,
                            object_kind: get(&r, "object_kind")?,
                            object_id: get(&r, "object_id")?,
                            happened_at: unsigned(get(&r, "happened_at")?)? / 1000,
                            result: get(&r, "result")?,
                            reason_code: get(&r, "reason_code")?,
                            change_summary,
                        })
                    })
                    .collect::<Result<Vec<_>, IdentityError>>()?;
                Ok(ConsoleResult::Audit(page(items, limit, |r| r.id)))
            }
            ConsoleQuery::Deliveries { after, limit } => {
                let rows=self.identity_rows("SELECT d.id,d.event_id,d.status,d.attempt_count,d.next_attempt_at,d.last_error_code,d.created_at,d.delivered_at,e.application_id,e.kind,a.name FROM identity_logout_delivery d JOIN identity_oidc_rp_session p ON p.id=d.rp_session_id JOIN identity_app_environment e ON e.id=p.environment_id JOIN application a ON a.id=e.application_id WHERE ($1::uuid IS NULL OR d.id>$1) ORDER BY d.id LIMIT $2",vec![after.into(),i64::from(limit+1).into()]).await?;
                let items = rows
                    .into_iter()
                    .map(|r| {
                        Ok(ConsoleDelivery {
                            id: get(&r, "id")?,
                            event_id: get(&r, "event_id")?,
                            application_id: get(&r, "application_id")?,
                            name: get(&r, "name")?,
                            environment: get(&r, "kind")?,
                            status: get(&r, "status")?,
                            attempt_count: u32::try_from(get::<i32>(&r, "attempt_count")?)
                                .map_err(|_| IdentityError::DependencyUnavailable)?,
                            next_attempt_at: unsigned(get(&r, "next_attempt_at")?)? / 1000,
                            last_error_code: get(&r, "last_error_code")?,
                            created_at: unsigned(get(&r, "created_at")?)? / 1000,
                            delivered_at: get::<Option<i64>>(&r, "delivered_at")?
                                .map(unsigned)
                                .transpose()?
                                .map(|v| v / 1000),
                        })
                    })
                    .collect::<Result<Vec<_>, IdentityError>>()?;
                Ok(ConsoleResult::Deliveries(page(items, limit, |r| r.id)))
            }
        }
    }
    pub(crate) async fn identity_personal_consent_environment(
        &self,
        user_id: i32,
        consent_id: i32,
    ) -> Result<Option<(i32, Uuid)>, IdentityError> {
        self.identity_rows("SELECT e.application_id,e.id FROM oauth_consent c JOIN identity_app_environment e ON e.id=c.environment_id WHERE c.user_id=$1 AND c.id=$2",vec![user_id.into(),consent_id.into()]).await?.first().map(|r|Ok((get(r,"application_id")?,get(r,"id")?))).transpose()
    }
    pub(crate) async fn write_identity_console(
        &self,
        command: IdentityWrite,
    ) -> Result<IdentityWriteResult, IdentityError> {
        match command {
            IdentityWrite::WithdrawConsent {
                user_id,
                environment_id,
            } => {
                self.identity_execute(
                    "DELETE FROM oauth_consent WHERE user_id=$1 AND environment_id=$2",
                    vec![user_id.into(), environment_id.into()],
                )
                .await?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::RetryLogout {
                delivery_id,
                max_age_seconds,
            } => {
                let at = signed(now(self.clock.as_ref())?)?;
                let row=self.identity_one("SELECT status,created_at,lease_until FROM identity_logout_delivery WHERE id=$1 FOR UPDATE",vec![delivery_id.into()]).await?;
                let status = get::<String>(&row, "status")?;
                if status == "delivered" {
                    return Err(IdentityError::AlreadyConsumed);
                }
                if get::<i64>(&row, "created_at")? <= at - signed(max_age_seconds * 1000)? {
                    return Err(IdentityError::Expired);
                }
                if get::<Option<i64>>(&row, "lease_until")?.is_some_and(|until| until > at) {
                    return Err(IdentityError::Conflict);
                }
                self.identity_execute("UPDATE identity_logout_delivery SET status='pending',next_attempt_at=$2,lease_owner=NULL,lease_until=NULL WHERE id=$1",vec![delivery_id.into(),at.into()]).await?;
                Ok(IdentityWriteResult::Changed)
            }
            _ => Err(IdentityError::InvalidInput),
        }
    }
}
