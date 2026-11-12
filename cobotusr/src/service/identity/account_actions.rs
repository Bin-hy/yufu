//! Account writes reauthenticate their actor inside the committing transaction.
use super::authenticate_current_actor;
use crate::service::identity::{
    browser::BrowserWriteEvidence,
    repository::*,
    sso::{LoginFlowProof, SsoService},
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::service::security::repository::now;
use crate::{
    config::Config,
    domain::identity::{
        account::{
            AccountPassword, AccountSnapshot, InvitationPurpose, IssuedSecret, PersonKind,
            SecretDigest,
        },
        error::IdentityError,
        operation::IdentityOperation,
        session::{ActionFlowSnapshot, ActionTarget, FlowPurpose},
    },
};
use auth::{
    claims::IdentityProfile,
    security::{
        budget::{Clock, SecurityBudget},
        identity::SecretToken,
    },
};
use std::sync::Arc;
use uuid::Uuid;

pub struct AccountService {
    pub(super) repository: Arc<dyn IdentityRepository>,
    pub(super) clock: Arc<dyn Clock>,
    pub(super) config: Config,
}

#[derive(Clone, Copy)]
pub struct InvitationRequest {
    pub purpose: InvitationPurpose,
    pub target_user_id: Option<i32>,
    pub person_kind: PersonKind,
    pub developer_grant: bool,
}
pub struct IssuedAccountAction {
    pub id: Uuid,
    pub secret: IssuedSecret,
    pub expires_at: u64,
}
pub struct ActionExchange {
    pub action_id: Uuid,
    pub proof: IssuedSecret,
    pub expires_at: u64,
    pub oidc_flow_id: Option<Uuid>,
}
pub struct ActionProof<'a> {
    pub action_id: Uuid,
    pub proof: &'a SecretToken,
}
pub enum InvitationAcceptance<'a> {
    NewAccount {
        username: &'a str,
        password: &'a AccountPassword,
        nickname: &'a str,
    },
    ExistingAccount,
}
#[derive(serde::Serialize)]
pub struct AccountContinuation {
    pub user_id: i32,
    pub oidc_flow_id: Option<Uuid>,
    pub requires_login: bool,
}
#[derive(serde::Serialize)]
pub struct AccountActionView {
    pub action_id: Uuid,
    pub kind: &'static str,
    pub expires_at: u64,
    pub person_kind: Option<PersonKind>,
    pub developer_grant: bool,
    pub requires_existing_account: bool,
    pub oidc_flow_id: Option<Uuid>,
}

impl AccountService {
    pub async fn inspect_action(
        &self,
        action: ActionProof<'_>,
        browser_binding: &SecretDigest,
        budget: &SecurityBudget,
    ) -> Result<AccountActionView, IdentityError> {
        crate::service::identity::sso::validate_opaque(action.proof.expose_secret())?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &InspectAction {
                service: self,
                id: action.action_id,
                proof: secret_digest(action.proof),
                browser_binding,
            },
        )
        .await
    }
    pub fn new(
        repository: Arc<dyn IdentityRepository>,
        clock: Arc<dyn Clock>,
        config: Config,
    ) -> Result<Self, IdentityError> {
        config
            .validate_identity_policy()
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        Ok(Self {
            repository,
            clock,
            config,
        })
    }
    pub async fn issue_invitation(
        &self,
        evidence: &BrowserWriteEvidence,
        request: InvitationRequest,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<IssuedAccountAction, IdentityError> {
        if request.target_user_id.is_some_and(|id| id <= 0)
            || (request.purpose == InvitationPurpose::Qualification
                && request.target_user_id.is_none())
        {
            return Err(IdentityError::InvalidInput);
        }
        execute_identity(
            self.repository.as_ref(),
            budget,
            &IssueAction {
                service: self,
                evidence,
                request_id,
                kind: IssueKind::Invitation(request),
            },
        )
        .await
    }
    pub async fn issue_reset(
        &self,
        evidence: &BrowserWriteEvidence,
        user_id: i32,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<IssuedAccountAction, IdentityError> {
        if user_id <= 0 {
            return Err(IdentityError::InvalidInput);
        }
        execute_identity(
            self.repository.as_ref(),
            budget,
            &IssueAction {
                service: self,
                evidence,
                request_id,
                kind: IssueKind::Reset(user_id),
            },
        )
        .await
    }
    pub async fn revoke_action(
        &self,
        evidence: &BrowserWriteEvidence,
        target: ActionTarget,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<(), IdentityError> {
        if target.id().is_nil() || matches!(target, ActionTarget::LogoutConfirmation(_)) {
            return Err(IdentityError::InvalidInput);
        }
        execute_identity(
            self.repository.as_ref(),
            budget,
            &RevokeAction {
                service: self,
                evidence,
                target,
                request_id,
            },
        )
        .await
    }
    /// Accept only a proof of an existing OP authorization flow as continuation.
    /// Neither a redirect URL nor arbitrary authorization parameters are accepted.
    pub async fn exchange_action(
        &self,
        kind: &str,
        secret: &SecretToken,
        evidence: &BrowserWriteEvidence,
        continuation: Option<&LoginFlowProof>,
        budget: &SecurityBudget,
    ) -> Result<ActionExchange, IdentityError> {
        crate::service::identity::sso::validate_opaque(secret.expose_secret())?;
        if !matches!(kind, "invitation" | "password_reset") {
            return Err(IdentityError::InvalidInput);
        }
        execute_identity(
            self.repository.as_ref(),
            budget,
            &ExchangeAction {
                service: self,
                kind,
                digest: secret_digest(secret),
                evidence,
                continuation,
            },
        )
        .await
    }
    pub async fn accept_invitation(
        &self,
        evidence: &BrowserWriteEvidence,
        action: ActionProof<'_>,
        acceptance: InvitationAcceptance<'_>,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<AccountContinuation, IdentityError> {
        crate::service::identity::sso::validate_opaque(action.proof.expose_secret())?;
        let new_account = match acceptance {
            InvitationAcceptance::NewAccount {
                username,
                password,
                nickname,
            } => {
                validate_username(username)?;
                validate_profile(nickname, None)?;
                Some((
                    username.to_owned(),
                    hash_password(password).await?,
                    nickname.to_owned(),
                ))
            }
            InvitationAcceptance::ExistingAccount => None,
        };
        // Password hashing is local CPU work; the write keeps a fresh
        // two-second database deadline even when hashing was slow.
        let write_budget = budget.renewed()?;
        execute_identity(
            self.repository.as_ref(),
            &write_budget,
            &AcceptInvitation {
                service: self,
                evidence,
                action_id: action.action_id,
                proof: secret_digest(action.proof),
                new_account,
                request_id,
            },
        )
        .await
    }
    pub async fn update_profile(
        &self,
        evidence: &BrowserWriteEvidence,
        nickname: &str,
        email: Option<&str>,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<IdentityProfile, IdentityError> {
        validate_profile(nickname, email)?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &ProfileUpdate {
                service: self,
                evidence,
                nickname,
                email,
                request_id,
            },
        )
        .await
    }
    pub(super) fn sso(&self) -> Result<SsoService, IdentityError> {
        SsoService::new(
            self.repository.clone(),
            self.clock.clone(),
            self.config.clone(),
        )
    }
}

struct InspectAction<'a> {
    service: &'a AccountService,
    id: Uuid,
    proof: SecretDigest,
    browser_binding: &'a SecretDigest,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for InspectAction<'_> {
    type Output = AccountActionView;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        let at = now(self.service.clock.as_ref())?;
        let flow = transaction
            .load_action_flow(self.id)
            .await?
            .ok_or(IdentityError::NotFound)?;
        flow.verify(flow.target, self.browser_binding, &self.proof, at)?;
        let (person_kind, developer_grant, requires_existing_account) = match flow.target {
            ActionTarget::Invitation(id) => {
                let source = transaction
                    .load_invitation(id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                if source.revoked_at.is_some() {
                    return Err(IdentityError::AccessDenied);
                }
                if source.consumed_at.is_some() {
                    return Err(IdentityError::AlreadyConsumed);
                }
                if at >= source.expires_at {
                    return Err(IdentityError::Expired);
                }
                (
                    Some(source.person_kind),
                    source.developer_grant,
                    source.target_user_id.is_some()
                        || source.purpose == InvitationPurpose::Qualification,
                )
            }
            ActionTarget::PasswordReset(id) => {
                let reset = transaction
                    .load_reset(id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                let account = transaction
                    .load_account(reset.user_id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                reset.validate(&account, at)?;
                (None, false, false)
            }
            _ => return Err(IdentityError::Forbidden),
        };
        Ok(AccountActionView {
            action_id: self.id,
            kind: flow.target.kind(),
            expires_at: flow.expires_at,
            person_kind,
            developer_grant,
            requires_existing_account,
            oidc_flow_id: continuation(&flow)?,
        })
    }
}

pub(super) fn secret_digest(secret: &SecretToken) -> SecretDigest {
    SecretDigest::of(secret.expose_secret())
}
pub(super) fn validate_username(username: &str) -> Result<(), IdentityError> {
    if username.is_empty()
        || username.len() > 128
        || username.trim() != username
        || username.chars().any(char::is_control)
    {
        return Err(IdentityError::InvalidInput);
    }
    Ok(())
}
fn validate_profile(nickname: &str, email: Option<&str>) -> Result<(), IdentityError> {
    if nickname.trim().is_empty()
        || nickname.chars().count() > 128
        || nickname.chars().any(char::is_control)
    {
        return Err(IdentityError::InvalidInput);
    }
    if let Some(email) = email {
        let parts: Vec<_> = email.split('@').collect();
        if email.len() > 254
            || parts.len() != 2
            || parts.iter().any(|part| part.is_empty())
            || email.chars().any(char::is_whitespace)
            || email.chars().any(char::is_control)
        {
            return Err(IdentityError::InvalidInput);
        }
    }
    Ok(())
}
pub(super) async fn hash_password(
    password: &AccountPassword,
) -> Result<SecretToken, IdentityError> {
    if password.expose_secret().is_empty() || password.expose_secret().len() > 72 {
        return Err(IdentityError::InvalidInput);
    }
    let password = password.expose_secret().to_owned();
    let hash = super::super::password::hash(password).await?;
    SecretToken::new(hash).map_err(Into::into)
}
pub(super) fn current_digest(
    evidence: &BrowserWriteEvidence,
) -> Result<&SecretDigest, IdentityError> {
    evidence
        .current_center_digest
        .as_ref()
        .ok_or(IdentityError::Unauthenticated)
}
pub(super) async fn audit(
    transaction: &mut dyn IdentityTransaction,
    actor_id: Option<i32>,
    request_id: Uuid,
    action: &'static str,
    object_kind: &'static str,
    object_id: String,
    summary: serde_json::Value,
) -> Result<Uuid, IdentityError> {
    let id = Uuid::new_v4();
    transaction
        .append_audit(&AuditEvent {
            id,
            request_id,
            actor_id,
            action,
            object_kind,
            object_id,
            result: AuditResult::Success,
            reason: None,
            change_summary: summary,
            operation_id: None,
            commit_marker: Some(id),
        })
        .await?;
    Ok(id)
}

enum IssueKind {
    Invitation(InvitationRequest),
    Reset(i32),
}
struct IssueAction<'a> {
    service: &'a AccountService,
    evidence: &'a BrowserWriteEvidence,
    request_id: Uuid,
    kind: IssueKind,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for IssueAction<'_> {
    type Output = IssuedAccountAction;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        let target = match self.kind {
            IssueKind::Invitation(request) => request.target_user_id,
            IssueKind::Reset(user_id) => Some(user_id),
        };
        let scope =
            IdentityLockScope::new(target.map(IdentityLockKey::User).into_iter().collect())?;
        let actor = authenticate_current_actor(
            transaction,
            current_digest(self.evidence)?,
            now(self.service.clock.as_ref())?,
            &scope,
        )
        .await?;
        transaction
            .authorize_identity_operation(
                &actor,
                match self.kind {
                    IssueKind::Invitation(_) => IdentityOperation::InvitationIssue,
                    IssueKind::Reset(_) => IdentityOperation::PasswordResetIssue,
                },
            )
            .await?;
        let account = match target {
            Some(id) => Some(
                transaction
                    .load_account(id)
                    .await?
                    .ok_or(IdentityError::NotFound)?,
            ),
            None => None,
        };
        if let Some(account) = &account {
            crate::domain::identity::account::ensure_enabled(account)?;
        }
        let secret = IssuedSecret::from_entropy(rand::random());
        let id = Uuid::new_v4();
        let at = now(self.service.clock.as_ref())?;
        let (command, ttl, action_name, object_kind) = match self.kind {
            IssueKind::Invitation(request) => {
                if account
                    .as_ref()
                    .is_some_and(|account| account.person_kind != request.person_kind)
                {
                    return Err(IdentityError::InvalidInput);
                }
                let ttl = self.service.config.identity.invitation_ttl_seconds;
                (
                    IdentityWrite::IssueInvitation(InvitationWrite {
                        id,
                        digest: secret.digest(),
                        purpose: request.purpose,
                        target_user_id: request.target_user_id,
                        person_kind: request.person_kind,
                        developer_grant: request.developer_grant,
                        created_by: actor.account().user_id,
                        expires_at: at + ttl * 1_000,
                    }),
                    ttl,
                    "account.invitation.issue",
                    "invitation",
                )
            }
            IssueKind::Reset(user_id) => {
                let ttl = self.service.config.identity.password_reset_ttl_seconds;
                (
                    IdentityWrite::IssueReset(ResetWrite {
                        id,
                        user_id,
                        digest: secret.digest(),
                        auth_epoch_at_issue: account
                            .as_ref()
                            .ok_or(IdentityError::NotFound)?
                            .auth_epoch,
                        created_by: actor.account().user_id,
                        expires_at: at + ttl * 1_000,
                    }),
                    ttl,
                    "account.reset.issue",
                    "password_reset",
                )
            }
        };
        transaction.apply(command).await?;
        audit(
            transaction,
            Some(actor.account().user_id),
            self.request_id,
            action_name,
            object_kind,
            id.to_string(),
            serde_json::json!({"target_user_id":target,"expires_at":at + ttl * 1_000}),
        )
        .await?;
        Ok(IssuedAccountAction {
            id,
            secret,
            expires_at: at + ttl * 1_000,
        })
    }
}
struct RevokeAction<'a> {
    service: &'a AccountService,
    evidence: &'a BrowserWriteEvidence,
    target: ActionTarget,
    request_id: Uuid,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for RevokeAction<'_> {
    type Output = ();
    async fn apply(&self, transaction: &mut dyn IdentityTransaction) -> Result<(), IdentityError> {
        let key = match self.target {
            ActionTarget::Invitation(id) => IdentityLockKey::Invitation(id),
            ActionTarget::PasswordReset(id) => IdentityLockKey::PasswordReset(id),
            _ => return Err(IdentityError::InvalidInput),
        };
        let actor = authenticate_current_actor(
            transaction,
            current_digest(self.evidence)?,
            now(self.service.clock.as_ref())?,
            &IdentityLockScope::new(vec![key])?,
        )
        .await?;
        transaction
            .authorize_identity_operation(
                &actor,
                match self.target {
                    ActionTarget::Invitation(_) => IdentityOperation::InvitationRevoke,
                    ActionTarget::PasswordReset(_) => IdentityOperation::PasswordResetRevoke,
                    _ => return Err(IdentityError::InvalidInput),
                },
            )
            .await?;
        transaction
            .apply(match self.target {
                ActionTarget::Invitation(invitation_id) => {
                    IdentityWrite::RevokeInvitation { invitation_id }
                }
                ActionTarget::PasswordReset(reset_id) => IdentityWrite::RevokeReset { reset_id },
                _ => return Err(IdentityError::InvalidInput),
            })
            .await?;
        audit(
            transaction,
            Some(actor.account().user_id),
            self.request_id,
            "account.action.revoke",
            self.target.kind(),
            self.target.id().to_string(),
            serde_json::json!({"revoked":true}),
        )
        .await?;
        Ok(())
    }
}

struct ExchangeAction<'a> {
    service: &'a AccountService,
    kind: &'a str,
    digest: SecretDigest,
    evidence: &'a BrowserWriteEvidence,
    continuation: Option<&'a LoginFlowProof>,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for ExchangeAction<'_> {
    type Output = ActionExchange;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        let (target, mut expires_at, mut keys) = if self.kind == "invitation" {
            let invitation = transaction
                .invitation_by_digest(&self.digest)
                .await?
                .ok_or(IdentityError::NotFound)?;
            (
                ActionTarget::Invitation(invitation.id),
                invitation.expires_at,
                vec![IdentityLockKey::Invitation(invitation.id)],
            )
        } else {
            let reset = transaction
                .reset_by_digest(&self.digest)
                .await?
                .ok_or(IdentityError::NotFound)?;
            (
                ActionTarget::PasswordReset(reset.id),
                reset.expires_at,
                vec![
                    IdentityLockKey::User(reset.user_id),
                    IdentityLockKey::PasswordReset(reset.id),
                ],
            )
        };
        if let Some(flow) = self.continuation {
            keys.push(IdentityLockKey::Flow(flow.id));
        }
        transaction
            .lock_identity_scope(&IdentityLockScope::new(keys)?)
            .await?;
        let at = now(self.service.clock.as_ref())?;
        match target {
            ActionTarget::Invitation(id) => {
                let invitation = transaction
                    .load_invitation(id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                if invitation.revoked_at.is_some() {
                    return Err(IdentityError::AccessDenied);
                }
                if invitation.consumed_at.is_some() {
                    return Err(IdentityError::AlreadyConsumed);
                }
                if at >= invitation.expires_at {
                    return Err(IdentityError::Expired);
                }
            }
            ActionTarget::PasswordReset(id) => {
                let reset = transaction
                    .load_reset(id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                let account = transaction
                    .load_account(reset.user_id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                reset.validate(&account, at)?;
            }
            _ => return Err(IdentityError::InvalidInput),
        }
        let oidc_flow_id = if let Some(proof) = self.continuation {
            let flow = transaction
                .load_authorization_flow(proof.id)
                .await?
                .ok_or(IdentityError::NotFound)?;
            flow.handle
                .verify(FlowPurpose::Authorization, proof.proof.expose_secret(), at)?;
            if flow.browser_binding_digest != self.evidence.browser_binding_digest {
                return Err(IdentityError::Forbidden);
            }
            expires_at = expires_at.min(flow.handle.expires_at);
            Some(proof.id)
        } else {
            None
        };
        expires_at = expires_at.min(at + self.service.config.identity.flow_ttl_seconds * 1_000);
        let proof = IssuedSecret::from_entropy(rand::random());
        let action_id = Uuid::new_v4();
        let payload = oidc_flow_id.map_or_else(
            || serde_json::json!({}),
            |id| serde_json::json!({"oidc_flow_id":id}),
        );
        transaction
            .apply(IdentityWrite::CreateActionFlow(ActionFlowSnapshot {
                id: action_id,
                target,
                browser_binding_digest: self.evidence.browser_binding_digest.clone(),
                browser_proof_digest: proof.digest(),
                expires_at,
                consumed_at: None,
                request_payload: payload,
            }))
            .await?;
        Ok(ActionExchange {
            action_id,
            proof,
            expires_at,
            oidc_flow_id,
        })
    }
}

pub(super) async fn action_flow(
    transaction: &mut dyn IdentityTransaction,
    id: Uuid,
    proof: &SecretDigest,
    evidence: &BrowserWriteEvidence,
    at: u64,
) -> Result<ActionFlowSnapshot, IdentityError> {
    let flow = transaction
        .load_action_flow(id)
        .await?
        .ok_or(IdentityError::NotFound)?;
    flow.verify(flow.target, &evidence.browser_binding_digest, proof, at)?;
    Ok(flow)
}
pub(super) fn continuation(flow: &ActionFlowSnapshot) -> Result<Option<Uuid>, IdentityError> {
    flow.request_payload
        .get("oidc_flow_id")
        .map(|id| {
            id.as_str()
                .ok_or(IdentityError::DependencyUnavailable)
                .and_then(|id| {
                    Uuid::parse_str(id).map_err(|_| IdentityError::DependencyUnavailable)
                })
        })
        .transpose()
}
struct AcceptInvitation<'a> {
    service: &'a AccountService,
    evidence: &'a BrowserWriteEvidence,
    action_id: Uuid,
    proof: SecretDigest,
    new_account: Option<(String, SecretToken, String)>,
    request_id: Uuid,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for AcceptInvitation<'_> {
    type Output = AccountContinuation;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        let at = now(self.service.clock.as_ref())?;
        let flow = action_flow(transaction, self.action_id, &self.proof, self.evidence, at).await?;
        let ActionTarget::Invitation(invitation_id) = flow.target else {
            return Err(IdentityError::Forbidden);
        };
        let initial = transaction
            .load_invitation(invitation_id)
            .await?
            .ok_or(IdentityError::NotFound)?;
        let mut keys = vec![
            IdentityLockKey::Action(self.action_id),
            IdentityLockKey::Invitation(invitation_id),
        ];
        if let Some(id) = initial.target_user_id {
            keys.push(IdentityLockKey::User(id));
        }
        let scope = IdentityLockScope::new(keys)?;
        let existing = if self.new_account.is_none() {
            Some(
                authenticate_current_actor(transaction, current_digest(self.evidence)?, at, &scope)
                    .await?,
            )
        } else {
            transaction.lock_identity_scope(&scope).await?;
            None
        };
        let flow = action_flow(
            transaction,
            self.action_id,
            &self.proof,
            self.evidence,
            now(self.service.clock.as_ref())?,
        )
        .await?;
        let invitation = transaction
            .load_invitation(invitation_id)
            .await?
            .ok_or(IdentityError::NotFound)?;
        invitation.validate(
            now(self.service.clock.as_ref())?,
            existing.as_ref().map(|actor| actor.account().user_id),
        )?;
        let account: AccountSnapshot = match &self.new_account {
            Some((username, password_hash, nickname)) => {
                let hash = SecretToken::new(password_hash.expose_secret().to_owned())?;
                let IdentityWriteResult::Account(account) = transaction
                    .apply(IdentityWrite::CreateAccount(NewAccountWrite {
                        username: username.clone(),
                        password_hash: hash,
                        nickname: nickname.clone(),
                        person_kind: invitation.person_kind,
                        developer_enabled: invitation.developer_grant,
                    }))
                    .await?
                else {
                    return Err(IdentityError::DependencyUnavailable);
                };
                account
            }
            None => {
                let account = existing
                    .as_ref()
                    .ok_or(IdentityError::Unauthenticated)?
                    .account();
                if account.person_kind != invitation.person_kind {
                    return Err(IdentityError::Forbidden);
                }
                account.clone()
            }
        };
        transaction
            .apply(IdentityWrite::AcceptInvitation {
                invitation_id,
                user_id: account.user_id,
            })
            .await?;
        transaction
            .apply(IdentityWrite::ConsumeActionFlow {
                id: self.action_id,
                target: flow.target,
                browser_binding_digest: self.evidence.browser_binding_digest.clone(),
                browser_proof_digest: self.proof.clone(),
            })
            .await?;
        audit(transaction, existing.as_ref().map(|actor| actor.account().user_id), self.request_id, "account.invitation.accept", "account", account.user_id.to_string(), serde_json::json!({"created":self.new_account.is_some(),"developer_grant":invitation.developer_grant})).await?;
        Ok(AccountContinuation {
            user_id: account.user_id,
            oidc_flow_id: continuation(&flow)?,
            requires_login: self.new_account.is_some(),
        })
    }
}
struct ProfileUpdate<'a> {
    service: &'a AccountService,
    evidence: &'a BrowserWriteEvidence,
    nickname: &'a str,
    email: Option<&'a str>,
    request_id: Uuid,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for ProfileUpdate<'_> {
    type Output = IdentityProfile;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<IdentityProfile, IdentityError> {
        let actor = authenticate_current_actor(
            transaction,
            current_digest(self.evidence)?,
            now(self.service.clock.as_ref())?,
            &IdentityLockScope::new(vec![])?,
        )
        .await?;
        transaction
            .apply(IdentityWrite::UpdateProfile {
                user_id: actor.account().user_id,
                nickname: self.nickname.to_owned(),
                email: self.email.map(str::to_owned),
            })
            .await?;
        audit(
            transaction,
            Some(actor.account().user_id),
            self.request_id,
            "account.profile.update",
            "account",
            actor.account().user_id.to_string(),
            serde_json::json!({"profile_changed":true}),
        )
        .await?;
        transaction
            .load_profile(actor.account().user_id)
            .await?
            .ok_or(IdentityError::NotFound)
    }
}
