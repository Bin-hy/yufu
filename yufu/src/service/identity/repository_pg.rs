use super::*;
use crate::domain::identity::{
    account::{owner_is_qualified, stable_principal},
    admission::{AccessGeneration, AdmissionInput, AdmissionSources, evaluate_admission},
    application::AppEnvironmentKind,
};
use crate::entity::{
    application, identity_access_generation, identity_app_config_version, identity_app_environment,
    identity_app_member, identity_app_test_user, identity_sso_session, oauth_client, user,
};
use crate::service::security::repository::{PgSecurityRepository, PgSecurityTransaction, now};
use auth::security::budget::Clock;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter,
    Statement,
};
use std::sync::Arc;

pub struct PgIdentityRepository {
    pub(crate) security: PgSecurityRepository,
}
impl PgIdentityRepository {
    pub fn new(primary: DatabaseConnection, clock: Arc<dyn Clock>) -> Self {
        Self {
            security: PgSecurityRepository::new(primary, clock),
        }
    }
}

#[async_trait::async_trait]
impl IdentityRepository for PgIdentityRepository {
    async fn begin(
        &self,
        budget: &SecurityBudget,
    ) -> Result<Box<dyn IdentityTransaction>, IdentityError> {
        Ok(Box::new(self.security.begin_shared(budget).await?))
    }
}

#[async_trait::async_trait]
impl IdentityTransaction for PgSecurityTransaction {
    async fn audit_search(
        &mut self,
        filter: &crate::domain::identity::audit::AuditFilter,
        cursor: Option<&crate::domain::identity::audit::AuditCursor>,
        limit: u32,
    ) -> Result<crate::domain::identity::audit::AuditPage, IdentityError> {
        self.identity_audit_search(filter, cursor, limit).await
    }
    async fn associate_audit_application(
        &mut self,
        event_id: Uuid,
        application_id: i32,
        environment_id: Option<Uuid>,
    ) -> Result<(), IdentityError> {
        self.identity_associate_audit(event_id, application_id, environment_id)
            .await
    }
    async fn application_launch(
        &mut self,
        admission: &ManagedAdmission,
    ) -> Result<crate::domain::identity::application::LaunchUrl, IdentityError> {
        self.identity_application_launch(admission).await
    }
    async fn console_read(
        &mut self,
        query: crate::domain::identity::console::ConsoleQuery,
    ) -> Result<crate::domain::identity::console::ConsoleResult, IdentityError> {
        self.identity_console_read(query).await
    }
    async fn personal_consent_environment(
        &mut self,
        user_id: i32,
        consent_id: i32,
    ) -> Result<Option<(i32, Uuid)>, IdentityError> {
        self.identity_personal_consent_environment(user_id, consent_id)
            .await
    }
    async fn legacy_client_ids(
        &mut self,
        after: i32,
        limit: u32,
    ) -> Result<Vec<i32>, IdentityError> {
        self.identity_legacy_ids(after, limit).await
    }
    async fn legacy_workspace(&mut self, client_id: i32) -> Result<LegacyWorkspace, IdentityError> {
        self.identity_legacy_workspace(client_id).await
    }
    async fn logout_rp_reference(
        &mut self,
        reference: &auth::claims::LogoutReference,
    ) -> Result<Option<crate::entity::identity_oidc_rp_session::Model>, IdentityError> {
        use crate::entity::{identity_oidc_rp_session as rp, oauth_client};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        let Some(client) = self
            .run(
                oauth_client::Entity::find()
                    .filter(oauth_client::Column::ClientId.eq(&reference.client_id))
                    .one(&self.transaction),
            )
            .await?
        else {
            return Ok(None);
        };
        Ok(self
            .run(
                rp::Entity::find()
                    .filter(rp::Column::ProtocolSid.eq(reference.protocol_sid))
                    .filter(rp::Column::Issuer.eq(&reference.issuer))
                    .filter(rp::Column::Subject.eq(&reference.subject))
                    .filter(rp::Column::OauthClientId.eq(client.id))
                    .filter(rp::Column::AuthTime.eq(signed(reference.auth_time)?))
                    .one(&self.transaction),
            )
            .await?)
    }
    async fn group_workspace(&mut self, group_id: Uuid) -> Result<GroupWorkspace, IdentityError> {
        self.identity_group_workspace(group_id).await
    }
    async fn group_ids(
        &mut self,
        after: Option<Uuid>,
        limit: u32,
        search: Option<&str>,
    ) -> Result<Vec<Uuid>, IdentityError> {
        self.identity_group_ids(after, limit, search).await
    }
    async fn assignment_targets(
        &mut self,
        application_id: i32,
    ) -> Result<Vec<AssignmentTarget>, IdentityError> {
        self.identity_assignment_targets(application_id).await
    }
    async fn assignment_target_summaries(
        &mut self,
        application_id: i32,
    ) -> Result<Vec<AssignmentTargetSummary>, IdentityError> {
        self.identity_assignment_target_summaries(application_id)
            .await
    }
    async fn live_rp_ids(&mut self, ids: &[Uuid]) -> Result<Vec<Uuid>, IdentityError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        if ids.len() > 10_000 {
            return Err(IdentityError::CapacityExceeded);
        }
        let placeholders = (1..=ids.len())
            .map(|i| format!("${i}"))
            .collect::<Vec<_>>()
            .join(",");
        self.identity_rows(&format!("SELECT id FROM identity_oidc_rp_session WHERE revoked_at IS NULL AND id IN ({placeholders}) ORDER BY id"),ids.iter().map(|id|(*id).into()).collect()).await?.into_iter().map(|row|get(&row,"id")).collect()
    }
    async fn approval_workspace(
        &mut self,
        approval_id: Uuid,
    ) -> Result<ApprovalWorkspace, IdentityError> {
        self.identity_approval_workspace(approval_id).await
    }
    async fn collaboration_visible(
        &mut self,
        requester_id: i32,
        candidate_id: i32,
    ) -> Result<bool, IdentityError> {
        self.identity_collaboration_visible(requester_id, candidate_id)
            .await
    }
    async fn collaboration_candidates(
        &mut self,
        requester_id: i32,
        after: i32,
        limit: u32,
        search: Option<&str>,
    ) -> Result<Vec<CollaborationCandidate>, IdentityError> {
        self.identity_collaboration_candidates(requester_id, after, limit, search)
            .await
    }
    async fn administrator_application_ids(
        &mut self,
        after: i32,
        limit: u32,
    ) -> Result<Vec<i32>, IdentityError> {
        self.identity_rows("SELECT id FROM application WHERE management_state='managed' AND id>$1 ORDER BY id LIMIT $2",vec![after.into(),i64::from(limit).into()]).await?.into_iter().map(|r|get(&r,"id")).collect()
    }
    async fn application_workspace(
        &mut self,
        application_id: i32,
    ) -> Result<ApplicationWorkspace, IdentityError> {
        self.identity_application_workspace(application_id).await
    }
    async fn developer_application_ids(
        &mut self,
        user_id: i32,
        after: i32,
        limit: u32,
    ) -> Result<Vec<i32>, IdentityError> {
        self.identity_developer_application_ids(user_id, after, limit)
            .await
    }
    async fn reader_summaries(
        &mut self,
        environment_id: Uuid,
        after: Option<Uuid>,
        limit: u32,
    ) -> Result<Vec<ReaderSummary>, IdentityError> {
        self.identity_reader_summaries(environment_id, after, limit)
            .await
    }
    async fn operation_receipt_by_id(
        &mut self,
        actor_id: i32,
        operation_id: Uuid,
    ) -> Result<Option<SafeOperationReceipt>, IdentityError> {
        self.identity_operation_receipt_by_id(actor_id, operation_id)
            .await
    }
    async fn approved_spa_origin(&mut self, origin: &str) -> Result<bool, IdentityError> {
        let rows=self.identity_rows("SELECT c.client_id FROM identity_app_environment e JOIN oauth_client c ON c.id=e.oauth_client_id AND c.environment_id=e.id JOIN identity_app_config_version v ON v.id=e.active_version_id AND v.environment_id=e.id WHERE c.management_state='managed' AND c.public_client AND v.payload->>'client_kind'='browser_spa' AND (v.payload->'browser_origins') ? $1",vec![origin.into()]).await?;
        for row in rows {
            let target = self
                .authorization_target(&get::<String>(&row, "client_id")?, None)
                .await?;
            if target.enabled
                && target
                    .config
                    .browser_origins
                    .iter()
                    .any(|o| o.as_str() == origin)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    async fn introspection_reader(
        &mut self,
        reader_id: &str,
    ) -> Result<Option<crate::entity::identity_introspection_reader::Model>, IdentityError> {
        use crate::entity::identity_introspection_reader as reader;
        Ok(self
            .run(
                reader::Entity::find()
                    .filter(reader::Column::ReaderId.eq(reader_id))
                    .one(&self.transaction),
            )
            .await?)
    }
    async fn environment_client(
        &mut self,
        environment_id: Uuid,
    ) -> Result<Option<oauth_client::Model>, IdentityError> {
        let env = self
            .run(
                identity_app_environment::Entity::find_by_id(environment_id).one(&self.transaction),
            )
            .await?;
        match env {
            Some(env) => Ok(self
                .run(oauth_client::Entity::find_by_id(env.oauth_client_id).one(&self.transaction))
                .await?),
            None => Ok(None),
        }
    }
    async fn application_maintainer(
        &mut self,
        application_id: i32,
        user_id: i32,
    ) -> Result<bool, IdentityError> {
        Ok(!self.identity_rows("SELECT 1 FROM identity_app_member WHERE application_id=$1 AND user_id=$2 AND member_role IN ('owner','maintainer')",
            vec![application_id.into(),user_id.into()]).await?.is_empty())
    }
    async fn protocol_revocation_target(
        &mut self,
        issuer: &str,
        token: &SecretToken,
        access: Option<&auth::security::credential::CredentialRef>,
    ) -> Result<Option<ProtocolRevocationTarget>, IdentityError> {
        self.identity_protocol_revocation_target(issuer, token, access)
            .await
    }
    async fn code_token_context(
        &mut self,
        code: &crate::entity::oauth_authorization_code::Model,
    ) -> Result<crate::domain::identity::session::IdentityTokenContext, IdentityError> {
        self.identity_code_token_context(code).await
    }
    async fn authorization_target(
        &mut self,
        client_id: &str,
        user_id: Option<i32>,
    ) -> Result<AuthorizationTarget, IdentityError> {
        self.identity_authorization_target(client_id, user_id).await
    }
    async fn current_consent(
        &mut self,
        user_id: i32,
        environment_id: Uuid,
        version_id: Uuid,
    ) -> Result<Option<IdentityScopeSet>, IdentityError> {
        self.identity_current_consent(user_id, environment_id, version_id)
            .await
    }
    async fn account_governance(
        &mut self,
        user_id: i32,
    ) -> Result<AccountGovernanceSnapshot, IdentityError> {
        self.identity_account_governance(user_id).await
    }
    async fn authorize_identity_operation(
        &mut self,
        actor: &crate::domain::identity::account::VerifiedActor,
        operation: crate::domain::identity::operation::IdentityOperation,
    ) -> Result<(), IdentityError> {
        let (method, route) = operation.registration();
        let rows = self.identity_rows(
            "SELECT EXISTS (SELECT 1 FROM \"user\" u JOIN authority a ON a.method=$3 AND a.authority_uri=$4 WHERE u.id=$1 AND u.enabled AND u.auth_epoch=$2 AND COALESCE((SELECT ua.allow FROM user_authority ua WHERE ua.user_id=u.id AND ua.authority_id=a.id), EXISTS (SELECT 1 FROM authority_role ar WHERE ar.role_id=u.role_id AND ar.authority_id=a.id) OR EXISTS (SELECT 1 FROM role r WHERE r.id=u.role_id AND r.name=$5))) AS allowed",
            vec![actor.account().user_id.into(), signed(actor.account().auth_epoch)?.into(), method.into(), route.into(), crate::entity::role::RoleEnum::Superadmin.get_name().into()],
        ).await?;
        if get::<bool>(
            rows.first().ok_or(IdentityError::DependencyUnavailable)?,
            "allowed",
        )? {
            Ok(())
        } else {
            Err(IdentityError::Forbidden)
        }
    }
    async fn lock_identity_scope(
        &mut self,
        scope: &IdentityLockScope,
    ) -> Result<(), IdentityError> {
        self.acquire_identity_scope(scope).await
    }
    async fn load_account(
        &mut self,
        user_id: i32,
    ) -> Result<Option<AccountSnapshot>, IdentityError> {
        self.identity_account(user_id).await
    }
    async fn load_browser_session(
        &mut self,
        digest: &SecretDigest,
    ) -> Result<Option<BrowserSessionSnapshot>, IdentityError> {
        self.run(
            identity_sso_session::Entity::find()
                .filter(identity_sso_session::Column::SecretDigest.eq(digest.as_bytes().to_vec()))
                .one(&self.transaction),
        )
        .await?
        .map(root_snapshot)
        .transpose()
    }
    async fn evaluate_admission(
        &mut self,
        query: &AdmissionQuery,
    ) -> Result<ManagedAdmission, IdentityError> {
        self.current_identity_admission(query).await
    }
    async fn evaluate_initial_admission(
        &mut self,
        query: &AdmissionQuery,
    ) -> Result<ManagedAdmission, IdentityError> {
        self.current_identity_admission_with_initial(query, true)
            .await
    }
    async fn approval_snapshot(
        &mut self,
        environment_id: Uuid,
        version_id: Uuid,
        logout_limitation_accepted: bool,
    ) -> Result<ApprovalSnapshot, IdentityError> {
        self.identity_approval_snapshot(environment_id, version_id, logout_limitation_accepted)
            .await
    }
    async fn operation_receipt(
        &mut self,
        actor_id: i32,
        kind: &str,
        key: &str,
    ) -> Result<Option<SafeOperationReceipt>, IdentityError> {
        let rows=self.identity_rows("SELECT * FROM identity_request_operation WHERE actor_id=$1 AND operation_kind=$2 AND idempotency_key=$3",vec![actor_id.into(),kind.into(),key.into()]).await?;
        rows.into_iter()
            .next()
            .map(|row| {
                let summary = get(&row, "result_summary")?;
                super::super::repository_outbox::validate_summary(&summary)?;
                Ok(SafeOperationReceipt {
                    operation_id: get(&row, "id")?,
                    request_hash: get(&row, "request_hash")?,
                    resource_id: get(&row, "resource_id")?,
                    summary,
                    committed_at: unsigned(get(&row, "committed_at")?)?,
                })
            })
            .transpose()
    }
    async fn invitation_by_digest(
        &mut self,
        digest: &SecretDigest,
    ) -> Result<Option<crate::domain::identity::account::InvitationSnapshot>, IdentityError> {
        self.identity_invitation(digest).await
    }
    async fn reset_by_digest(
        &mut self,
        digest: &SecretDigest,
    ) -> Result<Option<crate::domain::identity::account::PasswordResetSnapshot>, IdentityError>
    {
        self.identity_reset(digest).await
    }
    async fn load_action_flow(
        &mut self,
        id: Uuid,
    ) -> Result<Option<crate::domain::identity::session::ActionFlowSnapshot>, IdentityError> {
        self.identity_action(id).await
    }
    async fn load_invitation(
        &mut self,
        id: uuid::Uuid,
    ) -> Result<Option<crate::domain::identity::account::InvitationSnapshot>, IdentityError> {
        let rows = self
            .identity_rows(
                "SELECT secret_digest FROM identity_invitation WHERE id=$1",
                vec![id.into()],
            )
            .await?;
        match rows.first() {
            Some(row) => {
                self.identity_invitation(
                    &crate::domain::identity::account::SecretDigest::from_bytes(
                        get::<Vec<u8>>(row, "secret_digest")?
                            .try_into()
                            .map_err(|_| IdentityError::DependencyUnavailable)?,
                    ),
                )
                .await
            }
            None => Ok(None),
        }
    }
    async fn load_reset(
        &mut self,
        id: uuid::Uuid,
    ) -> Result<Option<crate::domain::identity::account::PasswordResetSnapshot>, IdentityError>
    {
        let rows = self
            .identity_rows(
                "SELECT secret_digest FROM identity_password_reset WHERE id=$1",
                vec![id.into()],
            )
            .await?;
        match rows.first() {
            Some(row) => {
                self.identity_reset(&crate::domain::identity::account::SecretDigest::from_bytes(
                    get::<Vec<u8>>(row, "secret_digest")?
                        .try_into()
                        .map_err(|_| IdentityError::DependencyUnavailable)?,
                ))
                .await
            }
            None => Ok(None),
        }
    }
    async fn load_profile(
        &mut self,
        user_id: i32,
    ) -> Result<Option<auth::claims::IdentityProfile>, IdentityError> {
        self.identity_profile(user_id).await
    }
    async fn load_password_account(
        &mut self,
        username: &str,
    ) -> Result<Option<PasswordAccount>, IdentityError> {
        let Some(user) = self
            .run(
                user::Entity::find()
                    .filter(user::Column::Username.eq(username))
                    .one(&self.transaction),
            )
            .await?
        else {
            return Ok(None);
        };
        let account = self
            .identity_account(user.id)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        Ok(Some(PasswordAccount {
            account,
            password_hash: SecretToken::new(user.password.ok_or(IdentityError::Unauthenticated)?)?,
        }))
    }
    async fn revocation_plan(
        &mut self,
        scope: &crate::domain::identity::session::IdentityRevocationScope,
    ) -> Result<IdentityRevocationPlan, IdentityError> {
        self.plan_identity_revocation(scope).await
    }
    async fn load_authorization_flow(
        &mut self,
        id: Uuid,
    ) -> Result<Option<crate::domain::identity::session::AuthorizationFlowSnapshot>, IdentityError>
    {
        self.identity_authorization_flow(id).await
    }
    async fn apply(
        &mut self,
        command: IdentityWrite,
    ) -> Result<IdentityWriteResult, IdentityError> {
        match command {
            command @ (IdentityWrite::WithdrawConsent { .. }
            | IdentityWrite::RetryLogout { .. }) => self.write_identity_console(command).await,
            command @ (IdentityWrite::RecordLegacyScan { .. }
            | IdentityWrite::AdoptLegacyApplication(_)) => {
                self.write_identity_migration(command).await
            }
            command @ (IdentityWrite::CreateIntrospectionReader { .. }
            | IdentityWrite::UpdateIntrospectionReader { .. }) => {
                self.write_identity_reader(command).await
            }
            command @ (IdentityWrite::CreateAccount(_)
            | IdentityWrite::UpdateProfile { .. }
            | IdentityWrite::SetAccount { .. }
            | IdentityWrite::ChangePassword { .. }
            | IdentityWrite::IssueInvitation(_)
            | IdentityWrite::AcceptInvitation { .. }
            | IdentityWrite::RevokeInvitation { .. }
            | IdentityWrite::RevokeReset { .. }
            | IdentityWrite::IssueReset(_)
            | IdentityWrite::CreateActionFlow(_)
            | IdentityWrite::ConsumeActionFlow { .. }
            | IdentityWrite::ConsumeReset { .. }
            | IdentityWrite::SetCandidate { .. }
            | IdentityWrite::CreateGroup { .. }
            | IdentityWrite::UpdateGroup { .. }
            | IdentityWrite::DeleteGroup { .. }
            | IdentityWrite::ChangeGroupMember { .. }) => {
                self.write_identity_account(command).await
            }
            command @ (IdentityWrite::RegisterApplication(_)
            | IdentityWrite::UpdateApplicationMetadata { .. }
            | IdentityWrite::SetMember { .. }
            | IdentityWrite::TransferOwner { .. }
            | IdentityWrite::InsertVersion(_)
            | IdentityWrite::CreateCandidateClient { .. }
            | IdentityWrite::RecordAdminActivation { .. }
            | IdentityWrite::SubmitApproval(_)
            | IdentityWrite::ReviewApproval { .. }
            | IdentityWrite::ResetEnvironmentSecret { .. }
            | IdentityWrite::ActivateVersion { .. }
            | IdentityWrite::SetApplicationLifecycle { .. }) => {
                self.write_identity_application(command).await
            }
            command @ (IdentityWrite::SetAssignment { .. }
            | IdentityWrite::SetTestUser { .. }
            | IdentityWrite::RecomputeGeneration { .. }
            | IdentityWrite::EnsureAccessGeneration { .. }) => {
                self.write_identity_admission(command).await
            }
            command @ (IdentityWrite::CreateCenter(_)
            | IdentityWrite::AttachManagementSession { .. }
            | IdentityWrite::AttachIdentitySession { .. }
            | IdentityWrite::CreateFlow { .. }
            | IdentityWrite::BindFlow { .. }
            | IdentityWrite::CompleteFlow { .. }
            | IdentityWrite::CreateRelyingParty { .. }
            | IdentityWrite::CreateCode { .. }
            | IdentityWrite::SaveConsent { .. }
            | IdentityWrite::Revoke(_)
            | IdentityWrite::RevokeAccountSessions { .. }) => {
                self.write_identity_session(command).await
            }
            IdentityWrite::SaveOperation(operation) => {
                self.write_identity_operation(operation).await
            }
        }
    }
    async fn append_audit(&mut self, event: &AuditEvent) -> Result<(), IdentityError> {
        self.write_identity_audit(event).await
    }
    async fn enqueue_logout(&mut self, event: &LogoutEvent) -> Result<(), IdentityError> {
        self.write_identity_logout(event).await
    }
}

impl PgSecurityTransaction {
    pub(crate) async fn identity_one(
        &self,
        sql: &str,
        values: Vec<sea_orm::Value>,
    ) -> Result<sea_orm::QueryResult, IdentityError> {
        self.budget
            .run(
                self.transaction
                    .query_one_raw(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        sql,
                        values,
                    )),
            )
            .await?
            .map_err(db_error)?
            .ok_or(IdentityError::NotFound)
    }
    pub(crate) async fn identity_rows(
        &self,
        sql: &str,
        values: Vec<sea_orm::Value>,
    ) -> Result<Vec<sea_orm::QueryResult>, IdentityError> {
        self.budget
            .run(
                self.transaction
                    .query_all_raw(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        sql,
                        values,
                    )),
            )
            .await?
            .map_err(db_error)
    }
    pub(crate) async fn acquire_identity_scope(
        &self,
        scope: &IdentityLockScope,
    ) -> Result<(), IdentityError> {
        if let Some(last) = self.last_identity_lock()? {
            for key in scope.keys() {
                if !self.identity_lock_held(key)? && key < &last {
                    return Err(IdentityError::Conflict);
                }
            }
        }
        for key in scope.keys() {
            if self
                .identity_locks
                .lock()
                .map_err(|_| IdentityError::DependencyUnavailable)?
                .contains(key)
            {
                continue;
            }
            // A shared hold is upgraded to the exclusive barrier below.
            self.identity_shared_locks
                .lock()
                .map_err(|_| IdentityError::DependencyUnavailable)?
                .remove(key);
            let (sql, values): (&str, Vec<sea_orm::Value>) = match key {
                IdentityLockKey::Policy => (
                    "UPDATE identity_policy_barrier SET revision=revision+1 WHERE id=1 RETURNING id",
                    vec![],
                ),
                IdentityLockKey::Application(id) => (
                    "UPDATE application SET identity_revision=identity_revision WHERE id=$1 RETURNING id",
                    vec![(*id).into()],
                ),
                IdentityLockKey::Environment(id) => (
                    "UPDATE identity_app_environment SET revision=revision WHERE id=$1 RETURNING id",
                    vec![(*id).into()],
                ),
                IdentityLockKey::OAuthClient(id) => (
                    "SELECT id FROM oauth_client WHERE id=$1 FOR UPDATE",
                    vec![(*id).into()],
                ),
                IdentityLockKey::Reader(id) => (
                    "UPDATE identity_introspection_reader SET secret_revision=secret_revision WHERE id=$1 RETURNING id",
                    vec![(*id).into()],
                ),
                IdentityLockKey::Group(id) => (
                    "UPDATE identity_user_group SET revision=revision WHERE id=$1 RETURNING id",
                    vec![(*id).into()],
                ),
                IdentityLockKey::User(id) => (
                    "UPDATE \"user\" SET identity_revision=identity_revision WHERE id=$1 RETURNING id",
                    vec![(*id).into()],
                ),
                IdentityLockKey::Center(id) => (
                    "SELECT id FROM identity_sso_session WHERE id=$1 FOR UPDATE",
                    vec![(*id).into()],
                ),
                IdentityLockKey::Flow(id) => (
                    "SELECT id FROM identity_authorization_flow WHERE id=$1 FOR UPDATE",
                    vec![(*id).into()],
                ),
                IdentityLockKey::Action(id) => (
                    "SELECT id FROM identity_action_flow WHERE id=$1 FOR UPDATE",
                    vec![(*id).into()],
                ),
                IdentityLockKey::Invitation(id) => (
                    "SELECT id FROM identity_invitation WHERE id=$1 FOR UPDATE",
                    vec![(*id).into()],
                ),
                IdentityLockKey::PasswordReset(id) => (
                    "SELECT id FROM identity_password_reset WHERE id=$1 FOR UPDATE",
                    vec![(*id).into()],
                ),
                IdentityLockKey::Code(id) => (
                    "SELECT id FROM oauth_authorization_code WHERE id=$1 FOR UPDATE",
                    vec![(*id).into()],
                ),
                IdentityLockKey::RelyingParty(id) => (
                    "SELECT id FROM identity_oidc_rp_session WHERE id=$1 FOR UPDATE",
                    vec![(*id).into()],
                ),
                IdentityLockKey::Grant(id) => (
                    "SELECT id FROM security_grant WHERE id=$1 FOR UPDATE",
                    vec![(*id).into()],
                ),
                IdentityLockKey::SecuritySession(id) => (
                    "SELECT id FROM security_session WHERE id=$1 FOR UPDATE",
                    vec![(*id).into()],
                ),
                IdentityLockKey::Family(id) => (
                    "SELECT id FROM security_refresh_family WHERE id=$1 FOR UPDATE",
                    vec![(*id).into()],
                ),
                IdentityLockKey::Credential { issuer, jti } => (
                    "SELECT jti FROM security_credential WHERE issuer=$1 AND jti=$2 FOR UPDATE",
                    vec![issuer.clone().into(), jti.clone().into()],
                ),
            };
            if self
                .run(
                    self.transaction
                        .query_one_raw(Statement::from_sql_and_values(
                            DbBackend::Postgres,
                            sql,
                            values,
                        )),
                )
                .await?
                .is_none()
            {
                return Err(IdentityError::NotFound);
            }
            self.identity_locks
                .lock()
                .map_err(|_| IdentityError::DependencyUnavailable)?
                .insert(key.clone());
        }
        Ok(())
    }
    fn identity_lock_held(&self, key: &IdentityLockKey) -> Result<bool, IdentityError> {
        Ok(self
            .identity_locks
            .lock()
            .map_err(|_| IdentityError::DependencyUnavailable)?
            .contains(key)
            || self
                .identity_shared_locks
                .lock()
                .map_err(|_| IdentityError::DependencyUnavailable)?
                .contains(key))
    }
    fn last_identity_lock(&self) -> Result<Option<IdentityLockKey>, IdentityError> {
        let exclusive = self
            .identity_locks
            .lock()
            .map_err(|_| IdentityError::DependencyUnavailable)?
            .last()
            .cloned();
        let shared = self
            .identity_shared_locks
            .lock()
            .map_err(|_| IdentityError::DependencyUnavailable)?
            .last()
            .cloned();
        Ok(exclusive.max(shared))
    }
    /// Mints hold the account row with `FOR SHARE`: concurrent logins for one
    /// account no longer collide with each other, while a revocation that
    /// commits after this snapshot still fails the lock with 40001 because it
    /// takes the exclusive write barrier on the same row.
    pub(crate) async fn acquire_shared_user_scope(
        &self,
        user_id: crate::IdType,
    ) -> Result<(), IdentityError> {
        let key = IdentityLockKey::User(user_id);
        if self.identity_lock_held(&key)? {
            return Ok(());
        }
        if let Some(last) = self.last_identity_lock()?
            && key < last
        {
            return Err(IdentityError::Conflict);
        }
        if self
            .run(
                self.transaction
                    .query_one_raw(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT id FROM \"user\" WHERE id=$1 FOR SHARE",
                        vec![user_id.into()],
                    )),
            )
            .await?
            .is_none()
        {
            return Err(IdentityError::NotFound);
        }
        self.identity_shared_locks
            .lock()
            .map_err(|_| IdentityError::DependencyUnavailable)?
            .insert(key);
        Ok(())
    }
    pub(crate) async fn current_identity_admission(
        &self,
        query: &AdmissionQuery,
    ) -> Result<ManagedAdmission, IdentityError> {
        self.current_identity_admission_with_initial(query, false)
            .await
    }
    async fn current_identity_admission_with_initial(
        &self,
        query: &AdmissionQuery,
        allow_initial: bool,
    ) -> Result<ManagedAdmission, IdentityError> {
        let root = self
            .run(
                identity_sso_session::Entity::find_by_id(query.sso_session_id)
                    .one(&self.transaction),
            )
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        let root = root_snapshot(root)?;
        let account = self
            .identity_account(root.user_id)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        let environment = self
            .run(
                identity_app_environment::Entity::find_by_id(query.environment_id)
                    .one(&self.transaction),
            )
            .await?
            .ok_or(IdentityError::AccessDenied)?;
        let app = self
            .run(application::Entity::find_by_id(environment.application_id).one(&self.transaction))
            .await?
            .ok_or(IdentityError::AccessDenied)?;
        let mode = management_mode(&app.management_mode)?;
        let kind = environment_kind(&environment.kind)?;
        let owner = self
            .run(
                identity_app_member::Entity::find()
                    .filter(identity_app_member::Column::ApplicationId.eq(app.id))
                    .filter(identity_app_member::Column::MemberRole.eq("owner"))
                    .one(&self.transaction),
            )
            .await?;
        let owner_qualified = if let Some(owner) = owner {
            self.identity_account(owner.user_id)
                .await?
                .is_some_and(|account| owner_is_qualified(&account, mode))
        } else {
            false
        };
        let client = self
            .run(
                oauth_client::Entity::find_by_id(environment.oauth_client_id)
                    .one(&self.transaction),
            )
            .await?
            .ok_or(IdentityError::AccessDenied)?;
        if client.management_state != "managed" || client.environment_id != Some(environment.id) {
            return Err(IdentityError::AccessDenied);
        }
        let binding = self
            .run(
                crate::entity::security_binding::session_client::Entity::find_by_id((
                    root.principal.issuer().to_owned(),
                    client.client_id.clone(),
                ))
                .one(&self.transaction),
            )
            .await?;
        let version_id = environment
            .active_version_id
            .ok_or(IdentityError::AccessDenied)?;
        let version = self
            .run(identity_app_config_version::Entity::find_by_id(version_id).one(&self.transaction))
            .await?
            .ok_or(IdentityError::AccessDenied)?;
        let config = EnvironmentConfig::from_payload(version.payload, kind, true)?;
        let sources = self
            .identity_sources(app.id, environment.id, &account, mode)
            .await?;
        let generation = self
            .run(
                identity_access_generation::Entity::find_by_id((environment.id, account.user_id))
                    .one(&self.transaction),
            )
            .await?;
        let generation = match generation {
            Some(generation) => AccessGeneration {
                generation: unsigned(generation.generation)?,
                eligible: generation.eligible,
            },
            None if allow_initial => AccessGeneration {
                generation: 0,
                eligible: account.enabled && sources.eligible(kind),
            },
            None => return Err(IdentityError::AccessDenied),
        };
        evaluate_admission(&AdmissionInput {
            account: &account,
            root: &root,
            now_millis: now(self.clock.as_ref())?,
            environment_id: environment.id,
            environment_kind: kind,
            application_active: app.management_state == "managed" && app.lifecycle == "active",
            owner_qualified,
            client_enabled: binding.is_some_and(|binding| {
                binding.enabled
                    && binding.application == app.app_key
                    && binding.audience == config.access_audience.as_str()
            }),
            active_version_id: Some(version_id),
            lifecycle_epoch: unsigned(environment.lifecycle_epoch)?,
            generation,
            sources,
            approved_scopes: &config.identity_scopes,
            requested_scopes: &query.requested_scopes,
        })
    }
    pub(crate) async fn identity_execute(
        &self,
        sql: &str,
        values: Vec<sea_orm::Value>,
    ) -> Result<u64, IdentityError> {
        Ok(self
            .budget
            .run(self.transaction.execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            )))
            .await?
            .map_err(db_error)?
            .rows_affected())
    }
    pub(crate) async fn identity_account(
        &self,
        user_id: i32,
    ) -> Result<Option<AccountSnapshot>, IdentityError> {
        let Some(user) = self
            .run(user::Entity::find_by_id(user_id).one(&self.transaction))
            .await?
        else {
            return Ok(None);
        };
        let row = self
            .identity_one(
                &format!(
                    "SELECT {} AS administrator FROM \"user\" u WHERE u.id=$2",
                    super::super::repository_account::ENTERPRISE_ADMINISTRATOR
                ),
                vec![
                    crate::entity::role::RoleEnum::Superadmin.get_name().into(),
                    user_id.into(),
                ],
            )
            .await?;
        Ok(Some(AccountSnapshot {
            user_id,
            enabled: user.enabled,
            person_kind: match user.person_kind.as_str() {
                "employee" => PersonKind::Employee,
                "collaborator" => PersonKind::Collaborator,
                _ => return Err(IdentityError::DependencyUnavailable),
            },
            developer_enabled: user.developer_enabled,
            administrator: row
                .try_get("", "administrator")
                .map_err(|_| IdentityError::DependencyUnavailable)?,
            auth_epoch: unsigned(user.auth_epoch)?,
        }))
    }
    pub(crate) async fn identity_sources(
        &self,
        application_id: i32,
        environment_id: Uuid,
        account: &AccountSnapshot,
        mode: ManagementMode,
    ) -> Result<AdmissionSources, IdentityError> {
        let row = self.run(self.transaction.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "SELECT EXISTS (SELECT 1 FROM identity_app_assignment WHERE application_id=$1 AND target_kind='user' AND user_id=$2) AS direct, (SELECT COUNT(*) FROM identity_app_assignment a JOIN identity_group_member m ON m.group_id=a.group_id WHERE a.application_id=$1 AND a.target_kind='group' AND m.user_id=$2) AS groups", [application_id.into(), account.user_id.into()]))).await?.ok_or(IdentityError::DependencyUnavailable)?;
        let member = self
            .run(
                identity_app_member::Entity::find_by_id((application_id, account.user_id))
                    .one(&self.transaction),
            )
            .await?
            .is_some();
        let test_user = self
            .run(
                identity_app_test_user::Entity::find_by_id((environment_id, account.user_id))
                    .one(&self.transaction),
            )
            .await?
            .is_some();
        Ok(AdmissionSources {
            direct_assignment: row
                .try_get("", "direct")
                .map_err(|_| IdentityError::DependencyUnavailable)?,
            group_assignments: usize::try_from(
                row.try_get::<i64>("", "groups")
                    .map_err(|_| IdentityError::DependencyUnavailable)?,
            )
            .map_err(|_| IdentityError::DependencyUnavailable)?,
            qualified_application_member: member && owner_is_qualified(account, mode),
            explicit_test_user: test_user,
        })
    }
}
pub(crate) fn unsigned(value: i64) -> Result<u64, IdentityError> {
    u64::try_from(value).map_err(|_| IdentityError::DependencyUnavailable)
}
pub(crate) fn signed(value: u64) -> Result<i64, IdentityError> {
    i64::try_from(value).map_err(|_| IdentityError::CapacityExceeded)
}
pub(crate) fn management_mode(value: &str) -> Result<ManagementMode, IdentityError> {
    match value {
        "developer_managed" => Ok(ManagementMode::DeveloperManaged),
        "admin_configured" => Ok(ManagementMode::AdminConfigured),
        _ => Err(IdentityError::DependencyUnavailable),
    }
}
pub(crate) fn environment_kind(value: &str) -> Result<AppEnvironmentKind, IdentityError> {
    match value {
        "development" => Ok(AppEnvironmentKind::Development),
        "production" => Ok(AppEnvironmentKind::Production),
        _ => Err(IdentityError::DependencyUnavailable),
    }
}
pub(crate) fn root_snapshot(
    row: identity_sso_session::Model,
) -> Result<BrowserSessionSnapshot, IdentityError> {
    Ok(BrowserSessionSnapshot {
        id: row.id,
        principal: stable_principal(&row.issuer, row.user_id)?,
        user_id: row.user_id,
        secret_digest: SecretDigest::from_bytes(
            row.secret_digest
                .try_into()
                .map_err(|_| IdentityError::DependencyUnavailable)?,
        ),
        auth_epoch: unsigned(row.auth_epoch)?,
        authenticated_at: unsigned(row.authenticated_at)?,
        expires_at: unsigned(row.expires_at)?,
        revoked_at: row.revoked_at.map(unsigned).transpose()?,
    })
}

pub(crate) fn db_error(error: sea_orm::DbErr) -> IdentityError {
    if matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    ) {
        return IdentityError::Conflict;
    }
    if matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::ForeignKeyConstraintViolation(_))
    ) {
        return IdentityError::InvalidInput;
    }
    if let sea_orm::DbErr::Exec(sea_orm::RuntimeErr::SqlxError(e))
    | sea_orm::DbErr::Query(sea_orm::RuntimeErr::SqlxError(e)) = &error
        && e.as_database_error()
            .and_then(|e| e.code())
            .is_some_and(|code| {
                code == "23514" || code == "23502" || code == "22001" || code == "22P02"
            })
    {
        return IdentityError::InvalidInput;
    }
    crate::service::security::repository::unavailable(error).into()
}
pub(crate) fn get<T: sea_orm::TryGetable>(
    row: &sea_orm::QueryResult,
    column: &str,
) -> Result<T, IdentityError> {
    row.try_get("", column)
        .map_err(|_| IdentityError::DependencyUnavailable)
}
pub(crate) fn timestamp(millis: i64) -> Result<chrono::NaiveDateTime, IdentityError> {
    chrono::DateTime::from_timestamp_millis(millis)
        .map(|at| at.naive_utc())
        .ok_or(IdentityError::CapacityExceeded)
}
pub(crate) fn one_row(rows: u64) -> Result<(), IdentityError> {
    if rows == 1 {
        Ok(())
    } else {
        Err(IdentityError::Conflict)
    }
}
