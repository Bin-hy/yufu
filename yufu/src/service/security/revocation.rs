//! Explicit revocation scopes; receipts are returned only after primary commit.
use super::{
    lifecycle::{
        CredentialLifecycle, authenticate_session_client, status_for_snapshot,
        validate_refresh_roots, validate_session_access,
    },
    repository::{SecurityTransaction, now},
};
use auth::security::{
    budget::SecurityBudget,
    credential::{AuthenticatedContext, CredentialRef},
    error::SecurityError,
    identity::{PrincipalKind, PrincipalRef, SecretToken, TokenUse},
    policy::{PolicyEffect, evaluate_policy},
};

#[derive(Debug, Clone)]
pub enum RevocationScope {
    Credential(CredentialRef),
    CurrentSession,
    RefreshFamily,
    ApplicationGrants { application: String },
}
/// Resolved storage targets. Request data never supplies the acting principal.
pub enum StoredRevocationScope {
    Credential(CredentialRef),
    CurrentSession(uuid::Uuid),
    RefreshFamily(uuid::Uuid),
    ApplicationGrants(String),
    UserSessions,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, serde::Serialize)]
pub struct RevocationReceipt {
    pub committed_at: u64,
    pub effective_by: u64,
}
pub struct OidcRevocationRequest<'a> {
    pub token: &'a SecretToken,
    pub client_id: &'a str,
    pub client_secret: Option<&'a SecretToken>,
    pub access_reference: Option<&'a CredentialRef>,
}
enum Action<'a> {
    Logout {
        refresh: &'a SecretToken,
        issuer: &'a str,
        redirect: Option<(&'a str, &'a str)>,
    },
    Managed {
        context: &'a AuthenticatedContext,
        scope: &'a RevocationScope,
    },
    Kickout {
        context: &'a AuthenticatedContext,
        subject: &'a str,
    },
    Consent {
        context: &'a AuthenticatedContext,
        id: i32,
    },
    Oidc {
        request: &'a OidcRevocationRequest<'a>,
        issuer: &'a str,
    },
}
impl CredentialLifecycle {
    pub async fn logout_local(
        &self,
        refresh: &SecretToken,
        issuer: &str,
        redirect: Option<(&str, &str)>,
        budget: &SecurityBudget,
    ) -> Result<RevocationReceipt, SecurityError> {
        self.retry_revocation(
            &Action::Logout {
                refresh,
                issuer,
                redirect,
            },
            budget,
        )
        .await
    }
    pub async fn revoke_credentials(
        &self,
        manager: &AuthenticatedContext,
        scope: RevocationScope,
        budget: &SecurityBudget,
    ) -> Result<RevocationReceipt, SecurityError> {
        let budget = manager.budget().child(budget.remaining()?)?;
        self.retry_revocation(
            &Action::Managed {
                context: manager,
                scope: &scope,
            },
            &budget,
        )
        .await
    }
    pub async fn kickout_sessions(
        &self,
        manager: &AuthenticatedContext,
        subject: &str,
        budget: &SecurityBudget,
    ) -> Result<RevocationReceipt, SecurityError> {
        let budget = manager.budget().child(budget.remaining()?)?;
        self.retry_revocation(
            &Action::Kickout {
                context: manager,
                subject,
            },
            &budget,
        )
        .await
    }
    pub async fn revoke_consent(
        &self,
        manager: &AuthenticatedContext,
        consent_id: i32,
        budget: &SecurityBudget,
    ) -> Result<RevocationReceipt, SecurityError> {
        let budget = manager.budget().child(budget.remaining()?)?;
        self.retry_revocation(
            &Action::Consent {
                context: manager,
                id: consent_id,
            },
            &budget,
        )
        .await
    }
    pub async fn revoke_oidc(
        &self,
        request: OidcRevocationRequest<'_>,
        issuer: &str,
        budget: &SecurityBudget,
    ) -> Result<RevocationReceipt, SecurityError> {
        self.retry_revocation(
            &Action::Oidc {
                request: &request,
                issuer,
            },
            budget,
        )
        .await
    }
    async fn retry_revocation(
        &self,
        action: &Action<'_>,
        budget: &SecurityBudget,
    ) -> Result<RevocationReceipt, SecurityError> {
        for attempt in 0..4 {
            budget.remaining()?;
            match self.revoke_once(action, budget).await {
                Err(SecurityError::Conflict) if attempt < 3 => tokio::task::yield_now().await,
                result => return result,
            }
        }
        Err(SecurityError::Conflict)
    }
    async fn revoke_once(
        &self,
        action: &Action<'_>,
        budget: &SecurityBudget,
    ) -> Result<RevocationReceipt, SecurityError> {
        let mut txn = self.repository.begin(budget).await?;
        match action {
            Action::Logout {
                refresh,
                issuer,
                redirect,
            } => {
                if let Some((client_id, uri)) = redirect {
                    let client = txn
                        .oidc_client(client_id)
                        .await?
                        .ok_or(SecurityError::InvalidCredential)?;
                    if !crate::service::oidc::OidcService::client_has_redirect(&client, uri) {
                        return Err(SecurityError::InvalidResource);
                    }
                }
                let snapshot = txn
                    .lock_refresh(issuer, refresh)
                    .await?
                    .ok_or(SecurityError::InvalidCredential)?;
                let row = &snapshot.current.credential;
                if row.oidc_client_id.is_some()
                    || row.audience != "yufu"
                    || row.application != "yufu"
                    || row.token_use != "user_session"
                    || row.principal_kind != "user"
                    || row.revocation_parent_jti.is_some()
                {
                    return Err(SecurityError::InvalidCredential);
                }
                validate_refresh_roots(&snapshot.current)?;
                let paired = CredentialRef::new(
                    *issuer,
                    row.paired_access_jti
                        .as_deref()
                        .ok_or(SecurityError::InvalidCredential)?,
                )?;
                let access = txn
                    .credential(&paired)
                    .await?
                    .ok_or(SecurityError::InvalidCredential)?;
                validate_session_access(row, &access.current.credential)?;
                let principal = PrincipalRef::new(*issuer, PrincipalKind::User, &row.subject)?;
                txn.revoke_scope(
                    &principal,
                    &StoredRevocationScope::CurrentSession(
                        row.session_id.ok_or(SecurityError::InvalidCredential)?,
                    ),
                    now(self.clock.as_ref())?,
                )
                .await?;
            }
            Action::Managed { context, scope } => {
                txn.prepare_management_mutation(context.principal(), None)
                    .await?;
                self.check_manager(txn.as_mut(), context).await?;
                let claims = context.verified().claims();
                let target = match scope {
                    RevocationScope::Credential(reference) => {
                        let discovered = txn
                            .credential(reference)
                            .await?
                            .ok_or(SecurityError::Forbidden)?;
                        let row = &discovered.current.credential;
                        if row.issuer != context.principal().issuer()
                            || row.principal_kind != "user"
                            || row.subject != context.principal().subject()
                        {
                            return Err(SecurityError::Forbidden);
                        }
                        let snapshot = txn
                            .lock_credential(reference)
                            .await?
                            .ok_or(SecurityError::Forbidden)?;
                        let row = &snapshot.current.credential;
                        if row.issuer != context.principal().issuer()
                            || row.principal_kind != "user"
                            || row.subject != context.principal().subject()
                        {
                            return Err(SecurityError::Forbidden);
                        }
                        StoredRevocationScope::Credential(reference.clone())
                    }
                    RevocationScope::CurrentSession => StoredRevocationScope::CurrentSession(
                        parse_root(claims.session_id.as_deref())?,
                    ),
                    RevocationScope::RefreshFamily => StoredRevocationScope::RefreshFamily(
                        parse_root(claims.family_id.as_deref())?,
                    ),
                    RevocationScope::ApplicationGrants { application } => {
                        self.authorize_revocation_uri(
                            txn.as_mut(),
                            context,
                            "DELETE",
                            "/oidc/authorized-apps/{consent_id}",
                        )
                        .await?;
                        StoredRevocationScope::ApplicationGrants(application.clone())
                    }
                };
                txn.revoke_scope(context.principal(), &target, now(self.clock.as_ref())?)
                    .await?;
            }
            Action::Kickout { context, subject } => {
                let principal =
                    PrincipalRef::new(context.principal().issuer(), PrincipalKind::User, *subject)?;
                txn.prepare_management_mutation(context.principal(), Some(&principal))
                    .await?;
                self.check_manager(txn.as_mut(), context).await?;
                self.authorize_revocation_uri(txn.as_mut(), context, "POST", "/kickout")
                    .await?;
                txn.revoke_scope(
                    &principal,
                    &StoredRevocationScope::UserSessions,
                    now(self.clock.as_ref())?,
                )
                .await?;
            }
            Action::Consent { context, id } => {
                txn.prepare_management_mutation(context.principal(), None)
                    .await?;
                self.check_manager(txn.as_mut(), context).await?;
                self.authorize_revocation_uri(
                    txn.as_mut(),
                    context,
                    "DELETE",
                    "/oidc/authorized-apps/{consent_id}",
                )
                .await?;
                let app = txn.consent_application(context.principal(), *id).await?;
                txn.revoke_scope(
                    context.principal(),
                    &StoredRevocationScope::ApplicationGrants(app),
                    now(self.clock.as_ref())?,
                )
                .await?;
            }
            Action::Oidc { request, issuer } => {
                let client = txn
                    .oidc_client(request.client_id)
                    .await?
                    .ok_or(SecurityError::InvalidCredential)?;
                authenticate_session_client(&client, request.client_secret, "").await?;
                txn.renew_budget()?;
                let snapshot = match txn.lock_refresh(issuer, request.token).await? {
                    Some(snapshot) => Some(snapshot),
                    None => match request.access_reference {
                        Some(reference) if reference.issuer() == *issuer => {
                            txn.lock_credential(reference).await?
                        }
                        _ => None,
                    },
                };
                if let Some(snapshot) = snapshot {
                    let row = &snapshot.current.credential;
                    if row.oidc_client_id.as_deref() == Some(client.client_id.as_str())
                        && row.principal_kind == "user"
                        && row.token_use == "user_session"
                        && row.revocation_parent_jti.is_none()
                    {
                        let principal =
                            PrincipalRef::new(*issuer, PrincipalKind::User, &row.subject)?;
                        let at = now(self.clock.as_ref())?;
                        if row.credential_kind == "refresh" {
                            validate_refresh_roots(&snapshot.current)?;
                            let paired = CredentialRef::new(
                                *issuer,
                                row.paired_access_jti
                                    .as_deref()
                                    .ok_or(SecurityError::InvalidCredential)?,
                            )?;
                            let access = txn
                                .credential(&paired)
                                .await?
                                .ok_or(SecurityError::InvalidCredential)?;
                            validate_session_access(row, &access.current.credential)?;
                            txn.revoke_scope(
                                &principal,
                                &StoredRevocationScope::Credential(paired),
                                at,
                            )
                            .await?;
                        }
                        txn.revoke_scope(
                            &principal,
                            &StoredRevocationScope::Credential(snapshot.reference().clone()),
                            at,
                        )
                        .await?;
                    }
                }
            }
        }
        budget.remaining()?;
        match action {
            Action::Managed { context, .. }
            | Action::Kickout { context, .. }
            | Action::Consent { context, .. } => context.ensure_current()?,
            _ => (),
        }
        txn.commit().await?;
        let committed_at = now(self.clock.as_ref())?;
        Ok(RevocationReceipt {
            committed_at,
            effective_by: committed_at
                .checked_add(10_000)
                .ok_or(SecurityError::DependencyUnavailable)?,
        })
    }
    pub(super) async fn check_manager(
        &self,
        txn: &mut dyn SecurityTransaction,
        manager: &AuthenticatedContext,
    ) -> Result<(), SecurityError> {
        manager.ensure_current()?;
        if manager.principal().kind() != PrincipalKind::User
            || manager.token_use() != TokenUse::UserSession
            || manager.audience() != "yufu"
            || manager.application() != "yufu"
        {
            return Err(SecurityError::Forbidden);
        }
        let row = txn
            .lock_credential(manager.credential())
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        let (status, key) = status_for_snapshot(&row, now(self.clock.as_ref())?)?;
        if status.claims != *manager.verified().claims()
            || key != manager.verified().signing_key_id()
        {
            return Err(SecurityError::InvalidCredential);
        }
        Ok(())
    }
    pub(super) async fn authorize_revocation_uri(
        &self,
        txn: &mut dyn SecurityTransaction,
        manager: &AuthenticatedContext,
        method: &str,
        route: &str,
    ) -> Result<(), SecurityError> {
        let (policy, operation) = txn
            .uri_policy(manager.verified().claims(), method, route)
            .await?;
        if !matches!(evaluate_policy(&policy, &operation), PolicyEffect::Allow(_)) {
            return Err(SecurityError::Forbidden);
        }
        Ok(())
    }
}
fn parse_root(id: Option<&str>) -> Result<uuid::Uuid, SecurityError> {
    uuid::Uuid::parse_str(id.ok_or(SecurityError::InvalidCredential)?)
        .map_err(|_| SecurityError::InvalidCredential)
}
