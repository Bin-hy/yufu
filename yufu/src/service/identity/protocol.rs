use super::{
    repository::*,
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::{
    config::Config,
    domain::identity::{
        application::{ClientKind, ConsentPolicy},
        error::IdentityError,
        session::IdentityRevocationScope,
        token::{validate_refresh_scope, verify_s256},
    },
    service::security::{
        lifecycle::{
            CodeExchangeRequest, CredentialLifecycle, RefreshRequest, SessionTokenSet,
            SessionTokenSigner, authenticate_session_client,
        },
        repository::now,
    },
};
use auth::security::budget::{Clock, SecurityBudget};
use std::sync::Arc;
use uuid::Uuid;

#[path = "protocol_status.rs"]
mod status;
pub use status::ProtocolClientAuthentication;

pub struct ProtocolService {
    repository: Arc<dyn IdentityRepository>,
    lifecycle: CredentialLifecycle,
    clock: Arc<dyn Clock>,
    signer: Arc<dyn SessionTokenSigner>,
    config: Config,
}
pub struct IssuedIdentityTokens {
    pub tokens: SessionTokenSet,
    pub scope: String,
}
impl ProtocolService {
    pub async fn userinfo(
        &self,
        token: &auth::security::identity::SecretToken,
        verifier: &auth::token_manager::IdTokenSigner,
        budget: &SecurityBudget,
    ) -> Result<crate::domain::identity::userinfo::UserInfo, IdentityError> {
        self.userinfo_from_origin(token, verifier, None, budget)
            .await
    }
    pub async fn userinfo_from_origin(
        &self,
        token: &auth::security::identity::SecretToken,
        verifier: &auth::token_manager::IdTokenSigner,
        origin: Option<&str>,
        budget: &SecurityBudget,
    ) -> Result<crate::domain::identity::userinfo::UserInfo, IdentityError> {
        use crate::domain::identity::application::IdentityScopeSet;
        let mut transaction = self.repository.begin(budget).await?;
        let result = async {
            let access = self
                .lifecycle
                .identity_access_in_transaction(
                    transaction.as_mut(),
                    token,
                    &self.config.issuer(),
                    verifier,
                )
                .await
                .map_err(credential_error)?;
            let user_id = access
                .subject
                .parse::<i32>()
                .map_err(|_| IdentityError::AccessDenied)?;
            let scopes = match &access.context {
                Some(context) => {
                    let target = transaction
                        .authorization_target(&context.client_id, Some(user_id))
                        .await?;
                    validate_protocol_origin(&target.config, &self.config.issuer(), origin)?;
                    if target.config.consent_policy != ConsentPolicy::EnterprisePreconsent {
                        let consent = transaction
                            .current_consent(
                                user_id,
                                target.environment_id,
                                target.config_version_id,
                            )
                            .await?;
                        if !consent
                            .is_some_and(|scopes| context.admission.scopes().is_subset_of(&scopes))
                        {
                            return Err(IdentityError::AccessDenied);
                        }
                    }
                    context.admission.scopes().clone()
                }
                // Historical clients have no immutable per-session scope
                // record. Compatibility returns only the stable subject.
                None => {
                    if let Some(origin) = origin {
                        let issuer = url::Url::parse(&self.config.issuer())
                            .map_err(|_| IdentityError::DependencyUnavailable)?;
                        if origin != issuer.origin().ascii_serialization() {
                            return Err(IdentityError::InvalidInput);
                        }
                    }
                    IdentityScopeSet::parse("openid")?
                }
            };
            let profile = transaction
                .load_profile(user_id)
                .await?
                .ok_or(IdentityError::AccessDenied)?;
            Ok((
                crate::domain::identity::userinfo::UserInfo::from_profile(
                    access.subject,
                    &scopes,
                    profile,
                ),
                access.validity,
            ))
        }
        .await;
        match result {
            Ok((response, validity)) => {
                transaction.commit().await?;
                budget.remaining()?;
                validity.ensure_fresh(self.clock.as_ref())?;
                Ok(response)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }
    pub fn new(
        repository: Arc<dyn IdentityRepository>,
        lifecycle: CredentialLifecycle,
        clock: Arc<dyn Clock>,
        signer: Arc<dyn SessionTokenSigner>,
        config: Config,
    ) -> Result<Self, IdentityError> {
        config
            .validate_identity_policy()
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        Ok(Self {
            repository,
            lifecycle,
            clock,
            signer,
            config,
        })
    }
    pub async fn exchange(
        &self,
        request: CodeExchangeRequest<'_>,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<IssuedIdentityTokens, IdentityError> {
        self.exchange_from_origin(request, None, request_id, budget)
            .await
    }
    pub async fn exchange_from_origin(
        &self,
        request: CodeExchangeRequest<'_>,
        origin: Option<&str>,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<IssuedIdentityTokens, IdentityError> {
        super::sso::validate_opaque(request.code.expose_secret())
            .map_err(|_| IdentityError::AccessDenied)?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &Exchange {
                service: self,
                request,
                origin,
                request_id,
                budget,
            },
        )
        .await
    }
    pub async fn refresh(
        &self,
        request: RefreshRequest<'_>,
        scope: Option<&str>,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<IssuedIdentityTokens, IdentityError> {
        self.refresh_from_origin(request, scope, None, request_id, budget)
            .await
    }
    pub async fn refresh_from_origin(
        &self,
        request: RefreshRequest<'_>,
        scope: Option<&str>,
        origin: Option<&str>,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<IssuedIdentityTokens, IdentityError> {
        if request.refresh_token.expose_secret().len() != 64
            || !request
                .refresh_token
                .expose_secret()
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(IdentityError::AccessDenied);
        }
        match execute_identity(
            self.repository.as_ref(),
            budget,
            &Refresh {
                service: self,
                request,
                scope,
                origin,
                request_id,
                budget,
            },
        )
        .await?
        {
            RefreshOutcome::Issued(tokens) => Ok(*tokens),
            RefreshOutcome::Replay => Err(IdentityError::AccessDenied),
        }
    }
}

fn credential_error(error: auth::security::error::SecurityError) -> IdentityError {
    match error {
        auth::security::error::SecurityError::InvalidCredential
        | auth::security::error::SecurityError::CredentialRevoked => IdentityError::AccessDenied,
        error => error.into(),
    }
}

fn validate_protocol_origin(
    config: &crate::domain::identity::application::EnvironmentConfig,
    issuer: &str,
    origin: Option<&str>,
) -> Result<(), IdentityError> {
    let Some(origin) = origin else {
        return Ok(());
    };
    let issuer = url::Url::parse(issuer).map_err(|_| IdentityError::DependencyUnavailable)?;
    if origin == issuer.origin().ascii_serialization() {
        return Ok(());
    }
    if config.client_kind == crate::domain::identity::application::ClientKind::BrowserSpa
        && config.browser_origins.iter().any(|o| o.as_str() == origin)
    {
        Ok(())
    } else {
        Err(IdentityError::InvalidInput)
    }
}

struct Refresh<'a> {
    service: &'a ProtocolService,
    request: RefreshRequest<'a>,
    scope: Option<&'a str>,
    origin: Option<&'a str>,
    request_id: Uuid,
    budget: &'a SecurityBudget,
}
enum RefreshOutcome {
    Issued(Box<IssuedIdentityTokens>),
    Replay,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for Refresh<'_> {
    type Output = RefreshOutcome;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        let issuer = self.service.config.issuer();
        let initial = self
            .service
            .lifecycle
            .load_identity_refresh_in_transaction(transaction, &self.request, &issuer, false)
            .await
            .map_err(credential_error)?;
        let user_id = initial
            .context()
            .admission
            .principal()
            .subject()
            .parse::<i32>()
            .map_err(|_| IdentityError::AccessDenied)?;
        let scope = IdentityRevocationScope::Grant {
            grant_id: initial.grant_id(),
        };
        let plan = transaction.revocation_plan(&scope).await?;
        let target = transaction
            .authorization_target(self.request.client_id, Some(user_id))
            .await?;
        let mut keys = target.locks.keys().to_vec();
        keys.extend_from_slice(plan.locks.keys());
        transaction
            .lock_identity_scope(&IdentityLockScope::new(keys)?)
            .await?;
        let current_target = transaction
            .authorization_target(self.request.client_id, Some(user_id))
            .await?;
        let current_plan = transaction.revocation_plan(&scope).await?;
        if current_target.locks.keys() != target.locks.keys()
            || current_target.config_version_id != target.config_version_id
            || current_plan.locks.keys() != plan.locks.keys()
            || current_plan.rp_session_ids != plan.rp_session_ids
        {
            return Err(IdentityError::Conflict);
        }
        let client = transaction
            .oidc_client(self.request.client_id)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        if client.management_state != "managed"
            || client.environment_id != Some(current_target.environment_id)
            || client.public_client == current_target.config.permits_client_secret()
        {
            return Err(IdentityError::Unauthenticated);
        }
        authenticate_session_client(&client, self.request.client_secret, "refresh_token").await?;
        transaction.renew_budget()?;
        validate_protocol_origin(&current_target.config, &issuer, self.origin)?;
        let current = self
            .service
            .lifecycle
            .load_identity_refresh_in_transaction(transaction, &self.request, &issuer, true)
            .await
            .map_err(credential_error)?;
        if !current.same_binding(&initial) {
            return Err(IdentityError::Conflict);
        }
        // A consumed summary is evidence of replay even if its own expiry or
        // consent later changed. Its family is revoked in a committed result.
        if current.replayed() {
            self.service
                .lifecycle
                .revoke_identity_refresh_replay_in_transaction(transaction, &current)
                .await
                .map_err(credential_error)?;
            transaction.apply(IdentityWrite::Revoke(scope)).await?;
            let id = Uuid::new_v4();
            transaction.append_audit(&AuditEvent {id,request_id:self.request_id,actor_id:Some(user_id),action:"token.refresh.replay",object_kind:"grant",object_id:current.grant_id().to_string(),result:AuditResult::Success,reason:None,change_summary:serde_json::json!({"client_id":self.request.client_id,"affected_count":current_plan.rp_session_ids.len()}),operation_id:None,commit_marker:Some(id)}).await?;
            transaction
                .enqueue_logout(&LogoutEvent {
                    audit_event_id: id,
                    rp_session_ids: current_plan.rp_session_ids,
                })
                .await?;
            return Ok(RefreshOutcome::Replay);
        }
        if !current_target.enabled {
            return Err(IdentityError::AccessDenied);
        }
        validate_refresh_scope(current.context().admission.scopes(), self.scope)?;
        if current_target.config.consent_policy != ConsentPolicy::EnterprisePreconsent {
            let consent = transaction
                .current_consent(
                    user_id,
                    current_target.environment_id,
                    current_target.config_version_id,
                )
                .await?;
            if !consent
                .is_some_and(|scopes| current.context().admission.scopes().is_subset_of(&scopes))
            {
                return Err(IdentityError::AccessDenied);
            }
        }
        let profile = transaction
            .load_profile(user_id)
            .await?
            .ok_or(IdentityError::AccessDenied)?;
        let tokens = self
            .service
            .lifecycle
            .rotate_identity_refresh_in_transaction(
                transaction,
                &current,
                &profile,
                self.service.config.identity.access_token_ttl_seconds,
                self.service.signer.as_ref(),
                self.budget,
            )
            .await
            .map_err(credential_error)?;
        let id = Uuid::new_v4();
        transaction.append_audit(&AuditEvent {id,request_id:self.request_id,actor_id:Some(user_id),action:"token.refresh",object_kind:"grant",object_id:current.grant_id().to_string(),result:AuditResult::Success,reason:None,change_summary:serde_json::json!({"scope":current.context().admission.scopes().as_protocol_string(),"client_id":self.request.client_id}),operation_id:None,commit_marker:Some(id)}).await?;
        Ok(RefreshOutcome::Issued(Box::new(IssuedIdentityTokens {
            tokens,
            scope: current.context().admission.scopes().as_protocol_string(),
        })))
    }
}
struct Exchange<'a> {
    service: &'a ProtocolService,
    request: CodeExchangeRequest<'a>,
    origin: Option<&'a str>,
    request_id: Uuid,
    budget: &'a SecurityBudget,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for Exchange<'_> {
    type Output = IssuedIdentityTokens;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        let code = transaction
            .authorization_code(self.request.code)
            .await?
            .ok_or(IdentityError::AccessDenied)?;
        if code.client_id != self.request.client_id || code.code_digest.is_none() || code.used {
            return Err(IdentityError::AccessDenied);
        }
        let target = transaction
            .authorization_target(self.request.client_id, Some(code.user_id))
            .await?;
        let mut keys = target.locks.keys().to_vec();
        keys.push(IdentityLockKey::Center(
            code.sso_session_id.ok_or(IdentityError::AccessDenied)?,
        ));
        keys.push(IdentityLockKey::Code(code.id));
        keys.push(IdentityLockKey::RelyingParty(
            code.oidc_rp_session_id.ok_or(IdentityError::AccessDenied)?,
        ));
        transaction
            .lock_identity_scope(&IdentityLockScope::new(keys)?)
            .await?;
        let current_target = transaction
            .authorization_target(self.request.client_id, Some(code.user_id))
            .await?;
        if current_target.config_version_id != target.config_version_id
            || current_target.locks.keys() != target.locks.keys()
        {
            return Err(IdentityError::Conflict);
        }
        if !current_target.enabled {
            return Err(IdentityError::AccessDenied);
        }
        validate_protocol_origin(
            &current_target.config,
            &self.service.config.issuer(),
            self.origin,
        )?;
        let client = transaction
            .oidc_client(self.request.client_id)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        if client.management_state != "managed"
            || client.environment_id != Some(target.environment_id)
            || client.public_client == target.config.permits_client_secret()
        {
            return Err(IdentityError::Unauthenticated);
        }
        authenticate_session_client(&client, self.request.client_secret, "authorization_code")
            .await
            .map_err(|error| match error {
                auth::security::error::SecurityError::InvalidCredential => {
                    IdentityError::Unauthenticated
                }
                error => error.into(),
            })?;
        transaction.renew_budget()?;
        let code = transaction
            .authorization_code(self.request.code)
            .await?
            .ok_or(IdentityError::AccessDenied)?;
        let has_pkce = !code.code_challenge.is_empty() || !code.code_challenge_method.is_empty();
        if code.used
            || code.client_id != self.request.client_id
            || (has_pkce && code.code_challenge_method != "S256")
            || self.request.redirect_uri != code.redirect_uri.as_deref()
            || !self
                .request
                .redirect_uri
                .is_some_and(|uri| target.config.accepts_redirect(uri))
        {
            return Err(IdentityError::AccessDenied);
        }
        if current_target.config.client_kind == ClientKind::BrowserSpa && !has_pkce {
            return Err(IdentityError::AccessDenied);
        }
        if has_pkce {
            verify_s256(
                &code.code_challenge,
                self.request
                    .code_verifier
                    .ok_or(IdentityError::AccessDenied)?
                    .expose_secret(),
            )?;
        }
        let context = transaction.code_token_context(&code).await?;
        if context.admission.principal().issuer() != self.service.config.issuer() {
            return Err(IdentityError::AccessDenied);
        }
        if target.config.consent_policy != ConsentPolicy::EnterprisePreconsent {
            let consent = transaction
                .current_consent(
                    code.user_id,
                    target.environment_id,
                    target.config_version_id,
                )
                .await?;
            if !consent.is_some_and(|scopes| context.admission.scopes().is_subset_of(&scopes)) {
                return Err(IdentityError::AccessDenied);
            }
        }
        transaction
            .consume_code(code.id, now(self.service.clock.as_ref())?)
            .await
            .map_err(|error| match error {
                auth::security::error::SecurityError::InvalidCredential => {
                    IdentityError::AccessDenied
                }
                error => error.into(),
            })?;
        let profile = transaction
            .load_profile(code.user_id)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        let tokens = self
            .service
            .lifecycle
            .issue_identity_session_in_transaction(
                transaction,
                &context,
                &profile,
                self.service.config.identity.access_token_ttl_seconds,
                self.service.signer.as_ref(),
                self.budget,
            )
            .await?;
        let session_id = Uuid::parse_str(
            tokens
                .claims
                .session_id
                .as_deref()
                .ok_or(IdentityError::DependencyUnavailable)?,
        )
        .map_err(|_| IdentityError::DependencyUnavailable)?;
        if tokens.id_token.is_none() {
            return Err(IdentityError::DependencyUnavailable);
        }
        transaction
            .apply(IdentityWrite::AttachIdentitySession {
                session_id,
                rp_session_id: code.oidc_rp_session_id.ok_or(IdentityError::AccessDenied)?,
                admission: context.admission.clone(),
            })
            .await?;
        let id = Uuid::new_v4();
        transaction.append_audit(&AuditEvent {id,request_id:self.request_id,actor_id:Some(code.user_id),action:"token.exchange",object_kind:"authorization",object_id:code.oidc_rp_session_id.ok_or(IdentityError::AccessDenied)?.to_string(),result:AuditResult::Success,reason:None,change_summary:serde_json::json!({"scope":context.admission.scopes().as_protocol_string(),"client_id":context.client_id}),operation_id:None,commit_marker:Some(id)}).await?;
        Ok(IssuedIdentityTokens {
            tokens,
            scope: context.admission.scopes().as_protocol_string(),
        })
    }
}
