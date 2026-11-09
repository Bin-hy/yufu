use super::repository::pg::{get, one_row, signed};
use super::repository::{
    AccountGovernanceSnapshot, IdentityLockKey, IdentityLockScope, IdentityWrite,
    IdentityWriteResult,
};
use crate::domain::identity::account::{InvitationPurpose, PersonKind};
use crate::domain::identity::{account::stable_principal, error::IdentityError};
use crate::service::security::repository::{PgSecurityTransaction, SecurityTransaction, now};
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DbBackend, IsolationLevel, Statement,
    TransactionTrait,
};

// Entry/owner qualification is an aggregate. Every mutation separately checks
// its registered operation. The predicate deliberately honors per-user denies.
pub(crate) const ENTERPRISE_ADMINISTRATOR: &str = "EXISTS (SELECT 1 FROM authority a WHERE a.method IN ('POST','PATCH','DELETE') AND (a.authority_uri LIKE '/admin/%' OR (a.method='POST' AND a.authority_uri='/user')) AND COALESCE((SELECT ua.allow FROM user_authority ua WHERE ua.user_id=u.id AND ua.authority_id=a.id),EXISTS (SELECT 1 FROM authority_role ar WHERE ar.role_id=u.role_id AND ar.authority_id=a.id) OR EXISTS (SELECT 1 FROM role r WHERE r.id=u.role_id AND r.name=$1)))";

/// Initialization records identity namespaces, not authentication evidence.
/// Its timestamp is provided by the host; no sessions or credentials are minted.
pub async fn initialize_provider(
    db: &DatabaseConnection,
    issuer: &str,
    initialized_at: u64,
) -> Result<(), IdentityError> {
    stable_principal(issuer, 1)?;
    let at = i64::try_from(initialized_at).map_err(|_| IdentityError::InvalidInput)?;
    let transaction = db
        .begin_with_config(
            Some(IsolationLevel::RepeatableRead),
            Some(AccessMode::ReadWrite),
        )
        .await
        .map_err(|_| IdentityError::DependencyUnavailable)?;
    transaction
        .execute_unprepared("LOCK TABLE identity_provider IN EXCLUSIVE MODE")
        .await
        .map_err(|_| IdentityError::DependencyUnavailable)?;
    let provider = transaction
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT issuer FROM identity_provider WHERE id=1".to_owned(),
        ))
        .await
        .map_err(|_| IdentityError::DependencyUnavailable)?;
    if let Some(provider) = provider {
        let persisted: String = provider
            .try_get("", "issuer")
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        if persisted != issuer {
            transaction
                .rollback()
                .await
                .map_err(|_| IdentityError::DependencyUnavailable)?;
            // Keep an operator-visible conflict without changing the fixed namespace.
            db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
                "INSERT INTO identity_migration_conflict (id,conflict_kind,evidence_summary,created_at) SELECT $1,'issuer_mismatch',$2,$3 WHERE NOT EXISTS (SELECT 1 FROM identity_migration_conflict WHERE conflict_kind='issuer_mismatch' AND resolved_at IS NULL)",
                [uuid::Uuid::new_v4().into(), serde_json::json!({"persisted_issuer":persisted,"configured_issuer":issuer}).into(), at.into()])).await.map_err(|_| IdentityError::DependencyUnavailable)?;
            return Err(IdentityError::Conflict);
        }
    } else {
        transaction
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO identity_provider (id,issuer,initialized_at) VALUES (1,$1,$2)",
                [issuer.into(), at.into()],
            ))
            .await
            .map_err(|_| IdentityError::DependencyUnavailable)?;
    }
    // An existing tombstone must never be silently attached to a new account.
    let conflict = transaction.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "SELECT 1 FROM \"user\" u JOIN identity_subject s ON s.issuer=$1 AND s.sub=u.id::text WHERE s.user_id IS DISTINCT FROM u.id OR s.tombstoned_at IS NOT NULL LIMIT 1", [issuer.into()])).await.map_err(|_| IdentityError::DependencyUnavailable)?;
    if conflict.is_some() {
        transaction
            .rollback()
            .await
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        return Err(IdentityError::Conflict);
    }
    transaction.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO identity_subject (issuer,sub,user_id,created_at) SELECT $1,id::text,id,$2 FROM \"user\" ON CONFLICT (issuer,sub) DO NOTHING", [issuer.into(), at.into()])).await.map_err(|_| IdentityError::DependencyUnavailable)?;
    transaction
        .commit()
        .await
        .map_err(|_| IdentityError::CommitUncertain)
}

impl PgSecurityTransaction {
    pub(crate) async fn identity_account_governance(
        &self,
        user_id: i32,
    ) -> Result<AccountGovernanceSnapshot, IdentityError> {
        use super::repository::pg::{get, management_mode, unsigned};
        let account = self
            .identity_account(user_id)
            .await?
            .ok_or(IdentityError::NotFound)?;
        let row = self
            .identity_one(
                "SELECT identity_revision FROM \"user\" WHERE id=$1",
                vec![user_id.into()],
            )
            .await?;
        let revision = unsigned(get(&row, "identity_revision")?)?;
        let mut keys = vec![IdentityLockKey::User(user_id)];
        let mut enabled_administrator_ids = Vec::new();
        for row in self
            .identity_rows(
                &format!(
                    "SELECT u.id FROM \"user\" u WHERE u.enabled AND {} ORDER BY u.id",
                    ENTERPRISE_ADMINISTRATOR
                ),
                vec![crate::entity::role::RoleEnum::Superadmin.get_name().into()],
            )
            .await?
        {
            let id = get(&row, "id")?;
            enabled_administrator_ids.push(id);
            keys.push(IdentityLockKey::User(id));
        }
        let mut active_owned_modes = Vec::new();
        for row in self.identity_rows("SELECT a.id,a.management_mode FROM application a JOIN identity_app_member m ON m.application_id=a.id WHERE m.user_id=$1 AND m.member_role='owner' AND a.management_state='managed' AND a.lifecycle='active' ORDER BY a.id", vec![user_id.into()]).await? {
            keys.push(IdentityLockKey::Application(get(&row,"id")?));
            active_owned_modes.push(management_mode(&get::<String>(&row,"management_mode")?)?);
        }
        let mut affected_environments = Vec::new();
        for row in self.identity_rows("SELECT e.id,e.application_id FROM identity_app_environment e WHERE EXISTS (SELECT 1 FROM identity_app_member m WHERE m.application_id=e.application_id AND m.user_id=$1) OR EXISTS (SELECT 1 FROM identity_app_assignment a WHERE a.application_id=e.application_id AND (a.user_id=$1 OR a.group_id IN (SELECT group_id FROM identity_group_member WHERE user_id=$1))) OR EXISTS (SELECT 1 FROM identity_app_test_user t WHERE t.environment_id=e.id AND t.user_id=$1) OR EXISTS (SELECT 1 FROM identity_access_generation g WHERE g.environment_id=e.id AND g.user_id=$1) ORDER BY e.id", vec![user_id.into()]).await? {
            let environment_id = get(&row,"id")?;
            let application_id: i32 = get(&row,"application_id")?;
            affected_environments.push(environment_id);
            keys.extend([IdentityLockKey::Application(application_id), IdentityLockKey::Environment(environment_id)]);
            for group in self.identity_rows("SELECT group_id FROM identity_app_assignment WHERE application_id=$1 AND group_id IS NOT NULL", vec![application_id.into()]).await? {
                keys.push(IdentityLockKey::Group(get(&group,"group_id")?));
            }
            for owner in self.identity_rows("SELECT user_id FROM identity_app_member WHERE application_id=$1 AND member_role='owner'", vec![application_id.into()]).await? {
                keys.push(IdentityLockKey::User(get(&owner,"user_id")?));
            }
        }
        let mut live_rp_session_ids = Vec::new();
        for row in self.identity_rows("SELECT id FROM identity_oidc_rp_session WHERE subject=$1::text AND revoked_at IS NULL ORDER BY id",vec![user_id.into()]).await? {
            live_rp_session_ids.push(get(&row,"id")?);
        }
        Ok(AccountGovernanceSnapshot {
            account,
            revision,
            enabled_administrator_ids,
            active_owned_modes,
            affected_environments,
            live_rp_session_ids,
            locks: IdentityLockScope::new(keys)?,
        })
    }
    pub(crate) async fn identity_invitation(
        &self,
        digest: &crate::domain::identity::account::SecretDigest,
    ) -> Result<Option<crate::domain::identity::account::InvitationSnapshot>, IdentityError> {
        use super::repository::pg::{get, unsigned};
        let rows = self
            .identity_rows(
                "SELECT * FROM identity_invitation WHERE secret_digest=$1",
                vec![digest.as_bytes().to_vec().into()],
            )
            .await?;
        rows.into_iter()
            .next()
            .map(|row| {
                Ok(crate::domain::identity::account::InvitationSnapshot {
                    id: get(&row, "id")?,
                    purpose: match get::<String>(&row, "purpose")?.as_str() {
                        "onboarding" => InvitationPurpose::Onboarding,
                        "qualification" => InvitationPurpose::Qualification,
                        _ => return Err(IdentityError::DependencyUnavailable),
                    },
                    target_user_id: get(&row, "target_user_id")?,
                    person_kind: parse_person_kind(&get::<String>(&row, "person_kind")?)?,
                    developer_grant: get(&row, "developer_grant")?,
                    expires_at: unsigned(get(&row, "expires_at")?)?,
                    consumed_at: get::<Option<i64>>(&row, "consumed_at")?
                        .map(unsigned)
                        .transpose()?,
                    revoked_at: get::<Option<i64>>(&row, "revoked_at")?
                        .map(unsigned)
                        .transpose()?,
                })
            })
            .transpose()
    }
    pub(crate) async fn identity_reset(
        &self,
        digest: &crate::domain::identity::account::SecretDigest,
    ) -> Result<Option<crate::domain::identity::account::PasswordResetSnapshot>, IdentityError>
    {
        use super::repository::pg::{get, unsigned};
        let rows = self
            .identity_rows(
                "SELECT * FROM identity_password_reset WHERE secret_digest=$1",
                vec![digest.as_bytes().to_vec().into()],
            )
            .await?;
        rows.into_iter()
            .next()
            .map(|row| {
                Ok(crate::domain::identity::account::PasswordResetSnapshot {
                    id: get(&row, "id")?,
                    user_id: get(&row, "user_id")?,
                    auth_epoch_at_issue: unsigned(get(&row, "auth_epoch_at_issue")?)?,
                    expires_at: unsigned(get(&row, "expires_at")?)?,
                    consumed_at: get::<Option<i64>>(&row, "consumed_at")?
                        .map(unsigned)
                        .transpose()?,
                    revoked_at: get::<Option<i64>>(&row, "revoked_at")?
                        .map(unsigned)
                        .transpose()?,
                })
            })
            .transpose()
    }
    pub(crate) async fn identity_action(
        &self,
        id: uuid::Uuid,
    ) -> Result<Option<crate::domain::identity::session::ActionFlowSnapshot>, IdentityError> {
        use super::repository::pg::{get, unsigned};
        use crate::domain::identity::{
            account::SecretDigest,
            session::{ActionFlowSnapshot, ActionTarget},
        };
        let rows = self
            .identity_rows(
                "SELECT * FROM identity_action_flow WHERE id=$1",
                vec![id.into()],
            )
            .await?;
        rows.into_iter()
            .next()
            .map(|row| {
                Ok(ActionFlowSnapshot {
                    id: get(&row, "id")?,
                    target: match get::<String>(&row, "action_kind")?.as_str() {
                        "invitation" => ActionTarget::Invitation(get(&row, "invitation_id")?),
                        "password_reset" => {
                            ActionTarget::PasswordReset(get(&row, "password_reset_id")?)
                        }
                        "logout_confirmation" => {
                            ActionTarget::LogoutConfirmation(get(&row, "sso_session_id")?)
                        }
                        _ => return Err(IdentityError::DependencyUnavailable),
                    },
                    browser_binding_digest: SecretDigest::from_bytes(
                        get::<Vec<u8>>(&row, "browser_binding_digest")?
                            .try_into()
                            .map_err(|_| IdentityError::DependencyUnavailable)?,
                    ),
                    browser_proof_digest: SecretDigest::from_bytes(
                        get::<Vec<u8>>(&row, "browser_proof_digest")?
                            .try_into()
                            .map_err(|_| IdentityError::DependencyUnavailable)?,
                    ),
                    expires_at: unsigned(get(&row, "expires_at")?)?,
                    consumed_at: get::<Option<i64>>(&row, "consumed_at")?
                        .map(unsigned)
                        .transpose()?,
                    request_payload: get(&row, "request_payload")?,
                })
            })
            .transpose()
    }
    pub(crate) async fn identity_profile(
        &self,
        user_id: i32,
    ) -> Result<Option<auth::claims::IdentityProfile>, IdentityError> {
        use super::repository::pg::get;
        let rows = self.identity_rows("SELECT nickname,username,email,email_verified_at FROM \"user\" WHERE id=$1 AND enabled", vec![user_id.into()]).await?;
        rows.into_iter()
            .next()
            .map(|row| {
                let email: Option<String> = get(&row, "email")?;
                Ok(auth::claims::IdentityProfile {
                    name: get(&row, "nickname")?,
                    preferred_username: Some(get(&row, "username")?),
                    email_verified: if email.is_some() {
                        Some(get::<Option<i64>>(&row, "email_verified_at")?.is_some())
                    } else {
                        None
                    },
                    email,
                })
            })
            .transpose()
    }
    pub(crate) async fn write_identity_account(
        &mut self,
        command: IdentityWrite,
    ) -> Result<IdentityWriteResult, IdentityError> {
        let at = signed(now(self.clock.as_ref())?)?;
        match command {
            IdentityWrite::CreateActionFlow(flow) => {
                use crate::domain::identity::session::{ActionTarget, DEFAULT_FLOW_TTL_SECONDS};
                if flow.id.is_nil()
                    || flow.target.id().is_nil()
                    || flow.consumed_at.is_some()
                    || flow.expires_at <= at as u64
                    || flow.expires_at - at as u64 > DEFAULT_FLOW_TTL_SECONDS * 1_000
                    || !flow.request_payload.is_object()
                {
                    return Err(IdentityError::InvalidInput);
                }
                // Raw action secrets are never part of an action's persisted payload.
                let allowed: &[&str] = match flow.target {
                    ActionTarget::LogoutConfirmation(_) => &[
                        "client_id",
                        "post_logout_redirect_uri",
                        "state",
                        "rp_session_id",
                        "config_version_id",
                    ],
                    _ => &["oidc_flow_id"],
                };
                if flow.request_payload.as_object().is_none_or(|m| {
                    m.iter().any(|(k, v)| {
                        !allowed.contains(&k.as_str())
                            || !v.is_string()
                            || v.as_str().is_none_or(|s| s.len() > 2048)
                    })
                }) {
                    return Err(IdentityError::InvalidInput);
                }
                let (invitation, reset, root) = match flow.target {
                    ActionTarget::Invitation(id) => (Some(id), None, None),
                    ActionTarget::PasswordReset(id) => (None, Some(id), None),
                    ActionTarget::LogoutConfirmation(id) => (None, None, Some(id)),
                };
                if root.is_some() {
                    self.identity_execute("INSERT INTO identity_browser_binding (secret_digest,touched_at) VALUES ($1,$2) ON CONFLICT (secret_digest) DO UPDATE SET revision=identity_browser_binding.revision+1,touched_at=EXCLUDED.touched_at",vec![flow.browser_binding_digest.as_bytes().to_vec().into(),at.into()]).await?;
                    let row = self.identity_one("SELECT COUNT(*)::bigint AS pending FROM identity_action_flow WHERE action_kind='logout_confirmation' AND browser_binding_digest=$1 AND consumed_at IS NULL AND expires_at>$2",vec![flow.browser_binding_digest.as_bytes().to_vec().into(),at.into()]).await?;
                    if get::<i64>(&row, "pending")?
                        >= crate::domain::identity::session::MAX_PENDING_FLOWS as i64
                    {
                        return Err(IdentityError::CapacityExceeded);
                    }
                }
                self.identity_execute("INSERT INTO identity_action_flow (id,action_kind,invitation_id,password_reset_id,sso_session_id,request_payload,browser_binding_digest,browser_proof_digest,created_at,expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)", vec![flow.id.into(),flow.target.kind().into(),invitation.into(),reset.into(),root.into(),flow.request_payload.into(),flow.browser_binding_digest.as_bytes().to_vec().into(),flow.browser_proof_digest.as_bytes().to_vec().into(),at.into(),signed(flow.expires_at)?.into()]).await?;
                Ok(IdentityWriteResult::Resource(flow.id))
            }
            IdentityWrite::ConsumeActionFlow {
                id,
                target,
                browser_binding_digest,
                browser_proof_digest,
            } => {
                let flow = self
                    .identity_action(id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                flow.verify(
                    target,
                    &browser_binding_digest,
                    &browser_proof_digest,
                    at as u64,
                )?;
                one_row(self.identity_execute("UPDATE identity_action_flow SET consumed_at=$4 WHERE id=$1 AND browser_binding_digest=$2 AND browser_proof_digest=$3 AND consumed_at IS NULL AND expires_at>$4", vec![id.into(),browser_binding_digest.as_bytes().to_vec().into(),browser_proof_digest.as_bytes().to_vec().into(),at.into()]).await?)?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::CreateAccount(account) => {
                let user = self
                    .register_local_user(
                        &account.username,
                        &account.password_hash,
                        &account.nickname,
                    )
                    .await?;
                self.identity_execute(
                    "UPDATE \"user\" SET person_kind=$2,developer_enabled=$3 WHERE id=$1",
                    vec![
                        user.id.into(),
                        person_kind(account.person_kind).into(),
                        account.developer_enabled.into(),
                    ],
                )
                .await?;
                one_row(self.identity_execute("INSERT INTO identity_subject (issuer,sub,user_id,created_at) SELECT issuer,$1,$2,$3 FROM identity_provider WHERE id=1", vec![user.id.to_string().into(), user.id.into(), at.into()]).await?)?;
                Ok(IdentityWriteResult::Account(
                    self.identity_account(user.id)
                        .await?
                        .ok_or(IdentityError::DependencyUnavailable)?,
                ))
            }
            IdentityWrite::UpdateProfile {
                user_id,
                nickname,
                email,
            } => {
                one_row(self.identity_execute("UPDATE \"user\" SET nickname=$2,email_verified_at=CASE WHEN email IS DISTINCT FROM $3 THEN NULL ELSE email_verified_at END,email=$3,identity_revision=identity_revision+1 WHERE id=$1 AND enabled", vec![user_id.into(), nickname.into(), email.into()]).await?)?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::SetAccount {
                account,
                expected_revision,
            } => {
                one_row(self.identity_execute("UPDATE \"user\" SET enabled=$2,developer_enabled=$3,auth_epoch=auth_epoch + CASE WHEN enabled AND NOT $2 THEN 1 ELSE 0 END,identity_revision=identity_revision+1 WHERE id=$1 AND identity_revision=$4 AND auth_epoch=$5", vec![account.user_id.into(), account.enabled.into(), account.developer_enabled.into(), signed(expected_revision)?.into(), signed(account.auth_epoch)?.into()]).await?)?;
                Ok(IdentityWriteResult::Account(
                    self.identity_account(account.user_id)
                        .await?
                        .ok_or(IdentityError::DependencyUnavailable)?,
                ))
            }
            IdentityWrite::ChangePassword {
                user_id,
                password_hash,
                expected_auth_epoch,
            } => {
                one_row(self.identity_execute("UPDATE \"user\" SET password=$2,auth_epoch=auth_epoch+1,identity_revision=identity_revision+1 WHERE id=$1 AND auth_epoch=$3 AND enabled", vec![user_id.into(), password_hash.expose_secret().to_owned().into(), signed(expected_auth_epoch)?.into()]).await?)?;
                self.identity_execute("UPDATE identity_password_reset SET revoked_at=$2 WHERE user_id=$1 AND consumed_at IS NULL AND revoked_at IS NULL", vec![user_id.into(), at.into()]).await?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::IssueInvitation(invitation) => {
                self.identity_execute("INSERT INTO identity_invitation (id,secret_digest,purpose,target_user_id,person_kind,developer_grant,created_by,created_at,expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)", vec![invitation.id.into(), invitation.digest.as_bytes().to_vec().into(), match invitation.purpose { InvitationPurpose::Onboarding => "onboarding", InvitationPurpose::Qualification => "qualification" }.into(), invitation.target_user_id.into(), person_kind(invitation.person_kind).into(), invitation.developer_grant.into(), invitation.created_by.into(), at.into(), signed(invitation.expires_at)?.into()]).await?;
                Ok(IdentityWriteResult::Resource(invitation.id))
            }
            IdentityWrite::AcceptInvitation {
                invitation_id,
                user_id,
            } => {
                one_row(self.identity_execute("UPDATE identity_invitation SET consumed_at=$3,accepted_user_id=$2 WHERE id=$1 AND consumed_at IS NULL AND revoked_at IS NULL AND expires_at>$3 AND (target_user_id IS NULL OR target_user_id=$2)", vec![invitation_id.into(), user_id.into(), at.into()]).await?)?;
                self.identity_execute("UPDATE \"user\" SET developer_enabled=developer_enabled OR i.developer_grant,identity_revision=identity_revision+1 FROM identity_invitation i WHERE \"user\".id=$2 AND i.id=$1", vec![invitation_id.into(), user_id.into()]).await?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::RevokeInvitation { invitation_id } => {
                one_row(self.identity_execute("UPDATE identity_invitation SET revoked_at=$2 WHERE id=$1 AND consumed_at IS NULL AND revoked_at IS NULL", vec![invitation_id.into(), at.into()]).await?)?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::RevokeReset { reset_id } => {
                one_row(self.identity_execute("UPDATE identity_password_reset SET revoked_at=$2 WHERE id=$1 AND consumed_at IS NULL AND revoked_at IS NULL", vec![reset_id.into(), at.into()]).await?)?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::IssueReset(reset) => {
                self.identity_execute("INSERT INTO identity_password_reset (id,user_id,secret_digest,auth_epoch_at_issue,created_by,created_at,expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7)", vec![reset.id.into(), reset.user_id.into(), reset.digest.as_bytes().to_vec().into(), signed(reset.auth_epoch_at_issue)?.into(), reset.created_by.into(), at.into(), signed(reset.expires_at)?.into()]).await?;
                Ok(IdentityWriteResult::Resource(reset.id))
            }
            IdentityWrite::ConsumeReset {
                reset_id,
                expected_auth_epoch,
            } => {
                one_row(self.identity_execute("UPDATE identity_password_reset r SET consumed_at=$3 FROM \"user\" u WHERE r.id=$1 AND r.user_id=u.id AND u.enabled AND u.auth_epoch=$2 AND r.auth_epoch_at_issue=$2 AND r.consumed_at IS NULL AND r.revoked_at IS NULL AND r.expires_at>$3", vec![reset_id.into(), signed(expected_auth_epoch)?.into(), at.into()]).await?)?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::SetCandidate {
                requester_user_id,
                candidate_user_id,
                allowed,
                actor_id,
            } => {
                if allowed {
                    self.identity_execute("INSERT INTO identity_collaboration_candidate (requester_user_id,candidate_user_id,created_by,created_at) VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING", vec![requester_user_id.into(), candidate_user_id.into(), actor_id.into(), at.into()]).await?;
                } else {
                    self.identity_execute("DELETE FROM identity_collaboration_candidate WHERE requester_user_id=$1 AND candidate_user_id=$2", vec![requester_user_id.into(), candidate_user_id.into()]).await?;
                }
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::CreateGroup {
                id,
                name,
                description,
                actor_id,
            } => {
                self.identity_execute("INSERT INTO identity_user_group (id,name,description,created_by,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$5)", vec![id.into(), name.into(), description.into(), actor_id.into(), at.into()]).await?;
                Ok(IdentityWriteResult::Resource(id))
            }
            IdentityWrite::ChangeGroupMember {
                group_id,
                user_id,
                present,
                expected_revision,
                actor_id,
            } => {
                one_row(self.identity_execute("UPDATE identity_user_group SET revision=revision+1,updated_at=$3 WHERE id=$1 AND revision=$2", vec![group_id.into(), signed(expected_revision)?.into(), at.into()]).await?)?;
                if present {
                    self.identity_execute("INSERT INTO identity_group_member (group_id,user_id,created_by,created_at) VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING", vec![group_id.into(), user_id.into(), actor_id.into(), at.into()]).await?;
                } else {
                    self.identity_execute(
                        "DELETE FROM identity_group_member WHERE group_id=$1 AND user_id=$2",
                        vec![group_id.into(), user_id.into()],
                    )
                    .await?;
                }
                // The service recomputes all affected generations in this same transaction.
                self.identity_execute("UPDATE application SET identity_revision=identity_revision+1,access_revision=access_revision+1 WHERE id IN (SELECT application_id FROM identity_app_assignment WHERE group_id=$1)", vec![group_id.into()]).await?;
                self.identity_execute("UPDATE identity_app_approval SET status='obsolete' WHERE status='pending' AND environment_id IN (SELECT e.id FROM identity_app_environment e JOIN identity_app_assignment a ON a.application_id=e.application_id WHERE a.group_id=$1)", vec![group_id.into()]).await?;
                Ok(IdentityWriteResult::Revision(expected_revision + 1))
            }
            IdentityWrite::UpdateGroup {
                group_id,
                name,
                description,
                expected_revision,
            } => {
                one_row(self.identity_execute("UPDATE identity_user_group SET name=$3,description=$4,revision=revision+1,updated_at=$5 WHERE id=$1 AND revision=$2",vec![group_id.into(),signed(expected_revision)?.into(),name.into(),description.into(),at.into()]).await?)?;
                Ok(IdentityWriteResult::Revision(
                    expected_revision
                        .checked_add(1)
                        .ok_or(IdentityError::CapacityExceeded)?,
                ))
            }
            IdentityWrite::DeleteGroup {
                group_id,
                expected_revision,
            } => {
                // The caller removes each assignment and recomputes affected sources in this transaction.
                self.identity_execute(
                    "DELETE FROM identity_group_member WHERE group_id=$1",
                    vec![group_id.into()],
                )
                .await?;
                one_row(
                    self.identity_execute(
                        "DELETE FROM identity_user_group WHERE id=$1 AND revision=$2",
                        vec![group_id.into(), signed(expected_revision)?.into()],
                    )
                    .await?,
                )?;
                Ok(IdentityWriteResult::Changed)
            }
            _ => Err(IdentityError::InvalidInput),
        }
    }
}
fn person_kind(kind: PersonKind) -> &'static str {
    match kind {
        PersonKind::Employee => "employee",
        PersonKind::Collaborator => "collaborator",
    }
}
fn parse_person_kind(value: &str) -> Result<PersonKind, IdentityError> {
    match value {
        "employee" => Ok(PersonKind::Employee),
        "collaborator" => Ok(PersonKind::Collaborator),
        _ => Err(IdentityError::DependencyUnavailable),
    }
}
