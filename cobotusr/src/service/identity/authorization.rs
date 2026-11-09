//! Browser authorization is decided by current primary facts and committed atomically.
use super::{
    repository::*,
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::{
    config::Config,
    domain::identity::{
        account::{IssuedSecret, SecretDigest},
        application::{ConsentPolicy, ExactRedirectUri},
        authorization::{AuthorizationParameters, AuthorizationRequest},
        error::IdentityError,
        session::*,
    },
    service::security::repository::now,
};
use auth::security::{
    budget::{Clock, SecurityBudget},
    identity::SecretToken,
};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

pub struct AuthorizationService {
    repository: Arc<dyn IdentityRepository>,
    clock: Arc<dyn Clock>,
    config: Config,
}
pub struct AuthorizationResult {
    pub outcome: AuthorizationOutcome,
    /// HTTP adapter writes this only into the independent HttpOnly flow cookie.
    pub flow_proof: Option<IssuedSecret>,
}
pub struct AuthorizationFlowProof<'a> {
    pub id: Uuid,
    pub proof: &'a SecretToken,
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowInteraction {
    Login,
    Consent,
    Ready,
}
#[derive(Serialize)]
pub struct AuthorizationFlowSummary {
    pub flow_id: Uuid,
    pub application_name: String,
    pub scopes: String,
    pub interaction: FlowInteraction,
    pub expires_at: u64,
    pub username: Option<String>,
}
impl AuthorizationService {
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
    pub async fn authorize(
        &self,
        parameters: AuthorizationParameters,
        browser: &SecretDigest,
        center_secret: Option<&str>,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<AuthorizationResult, IdentityError> {
        let root_digest = center_secret
            .map(|secret| {
                super::sso::validate_opaque(secret)?;
                Ok::<_, IdentityError>(SecretDigest::of(secret))
            })
            .transpose()?;
        let unit = AuthorizationUnit {
            service: self,
            browser,
            root_digest,
            request_id,
            input: Input::Start(parameters),
            flow_id: Uuid::new_v4(),
            rp_id: Uuid::new_v4(),
            protocol_sid: Uuid::new_v4(),
            proof_entropy: rand::random(),
            code_entropy: rand::random(),
        };
        execute_identity(self.repository.as_ref(), budget, &unit).await
    }
    pub async fn complete(
        &self,
        flow: AuthorizationFlowProof<'_>,
        evidence: &super::browser::BrowserWriteEvidence,
        approved: bool,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<AuthorizationResult, IdentityError> {
        if evidence.current_center_digest.is_none() {
            return Err(IdentityError::Unauthenticated);
        }
        let unit = AuthorizationUnit {
            service: self,
            browser: &evidence.browser_binding_digest,
            root_digest: evidence.current_center_digest.clone(),
            request_id,
            input: Input::Complete { flow, approved },
            flow_id: Uuid::nil(),
            rp_id: Uuid::new_v4(),
            protocol_sid: Uuid::new_v4(),
            proof_entropy: rand::random(),
            code_entropy: rand::random(),
        };
        execute_identity(self.repository.as_ref(), budget, &unit).await
    }
    /// Reading a flow does not bind/consume it or rotate any browser evidence.
    pub async fn inspect(
        &self,
        proof: AuthorizationFlowProof<'_>,
        browser: &SecretDigest,
        center_secret: Option<&str>,
        budget: &SecurityBudget,
    ) -> Result<AuthorizationFlowSummary, IdentityError> {
        let mut transaction = self.repository.begin(budget).await?;
        let result = async {
            let flow = transaction
                .load_authorization_flow(proof.id)
                .await?
                .ok_or(IdentityError::NotFound)?;
            verify_flow(&flow, &proof, browser, now(self.clock.as_ref())?)?;
            let parameters = AuthorizationParameters::from_payload(flow.request_payload.clone())?;
            let request = parameters.validate()?;
            let digest = center_secret
                .map(|secret| {
                    super::sso::validate_opaque(secret)?;
                    Ok::<_, IdentityError>(SecretDigest::of(secret))
                })
                .transpose()?;
            let root = load_root(
                transaction.as_mut(),
                digest.as_ref(),
                &self.config.issuer(),
                now(self.clock.as_ref())?,
            )
            .await?;
            let target = transaction
                .authorization_target(&request.client_id, root.as_ref().map(|r| r.user_id))
                .await?;
            verify_version(&flow, &target)?;
            trusted_redirect(&target, &parameters)?;
            if !target.enabled {
                return Err(IdentityError::AccessDenied);
            }
            if let Some(root) = &root {
                if flow.bound_sso_session_id.is_some_and(|id| id != root.id) {
                    return Err(IdentityError::Forbidden);
                }
                transaction
                    .evaluate_initial_admission(&AdmissionQuery {
                        sso_session_id: root.id,
                        environment_id: target.environment_id,
                        requested_scopes: request.scopes.clone(),
                    })
                    .await?;
            }
            let consent =
                consent_current(transaction.as_mut(), &target, &request, root.as_ref()).await?;
            let interaction = match interaction_decision(
                request.prompt,
                request.max_age,
                root.as_ref().map(|r| r.authenticated_at / 1000),
                now(self.clock.as_ref())? / 1000,
                flow.reauth_completed_at.is_some(),
                consent,
            )? {
                InteractionDecision::Continue => FlowInteraction::Ready,
                InteractionDecision::Consent => FlowInteraction::Consent,
                InteractionDecision::Login => FlowInteraction::Login,
                _ => return Err(IdentityError::InvalidInput),
            };
            let username = match root {
                Some(root) => transaction
                    .load_profile(root.user_id)
                    .await?
                    .and_then(|p| p.preferred_username),
                None => None,
            };
            Ok(AuthorizationFlowSummary {
                flow_id: proof.id,
                application_name: target.application_name,
                scopes: request.scopes.as_protocol_string(),
                interaction,
                expires_at: flow.handle.expires_at / 1000,
                username,
            })
        }
        .await;
        transaction.rollback().await?;
        result
    }
}

enum Input<'a> {
    Start(AuthorizationParameters),
    Complete {
        flow: AuthorizationFlowProof<'a>,
        approved: bool,
    },
}
struct AuthorizationUnit<'a> {
    service: &'a AuthorizationService,
    browser: &'a SecretDigest,
    root_digest: Option<SecretDigest>,
    request_id: Uuid,
    input: Input<'a>,
    flow_id: Uuid,
    rp_id: Uuid,
    protocol_sid: Uuid,
    proof_entropy: [u8; 32],
    code_entropy: [u8; 32],
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for AuthorizationUnit<'_> {
    type Output = AuthorizationResult;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        let initial_flow = match &self.input {
            Input::Start(_) => None,
            Input::Complete { flow, .. } => {
                let current = transaction
                    .load_authorization_flow(flow.id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                verify_flow(
                    &current,
                    flow,
                    self.browser,
                    now(self.service.clock.as_ref())?,
                )?;
                Some(current)
            }
        };
        let parameters = match (&self.input, &initial_flow) {
            (Input::Start(parameters), _) => parameters.clone(),
            (_, Some(flow)) => AuthorizationParameters::from_payload(flow.request_payload.clone())?,
            _ => return Err(IdentityError::InvalidInput),
        };
        let root = load_root(
            transaction,
            self.root_digest.as_ref(),
            &self.service.config.issuer(),
            now(self.service.clock.as_ref())?,
        )
        .await?;
        let target = transaction
            .authorization_target(parameters.client_id()?, root.as_ref().map(|r| r.user_id))
            .await?;
        let mut keys = target.locks.keys().to_vec();
        if let Some(root) = &root {
            keys.push(IdentityLockKey::Center(root.id));
        }
        if let Some(flow) = &initial_flow {
            keys.push(IdentityLockKey::Flow(flow.handle.id()));
        }
        transaction
            .lock_identity_scope(&IdentityLockScope::new(keys)?)
            .await?;
        let current_target = transaction
            .authorization_target(parameters.client_id()?, root.as_ref().map(|r| r.user_id))
            .await?;
        if target.config_version_id != current_target.config_version_id
            || target.locks.keys() != current_target.locks.keys()
        {
            return Err(IdentityError::Conflict);
        }
        let target = current_target;
        let redirect = trusted_redirect(&target, &parameters)?;
        let request = match parameters.validate() {
            Ok(request) => request,
            Err(_) => {
                return Ok(protocol_error(
                    redirect,
                    parameters.optional_state(),
                    parameters.validation_error_code(),
                ));
            }
        };
        if !request.scopes.is_subset_of(&target.config.identity_scopes) {
            return Ok(protocol_error(
                redirect,
                Some(request.state),
                "invalid_scope",
            ));
        }
        if !target.enabled {
            return Ok(protocol_error(
                redirect,
                Some(request.state),
                "access_denied",
            ));
        }
        let current_root = load_root(
            transaction,
            self.root_digest.as_ref(),
            &self.service.config.issuer(),
            now(self.service.clock.as_ref())?,
        )
        .await?;
        if root.as_ref().map(|r| r.id) != current_root.as_ref().map(|r| r.id) {
            return Err(IdentityError::Conflict);
        }
        let root = current_root;
        let flow = match &self.input {
            Input::Start(_) => None,
            Input::Complete { flow, .. } => {
                let current = transaction
                    .load_authorization_flow(flow.id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                verify_flow(
                    &current,
                    flow,
                    self.browser,
                    now(self.service.clock.as_ref())?,
                )?;
                verify_version(&current, &target)?;
                if current
                    .bound_sso_session_id
                    .is_some_and(|id| Some(id) != root.as_ref().map(|r| r.id))
                {
                    return Err(IdentityError::Forbidden);
                }
                Some(current)
            }
        };
        let admission = if let Some(root) = &root {
            match transaction
                .evaluate_initial_admission(&AdmissionQuery {
                    sso_session_id: root.id,
                    environment_id: target.environment_id,
                    requested_scopes: request.scopes.clone(),
                })
                .await
            {
                Ok(admission) => Some(admission),
                Err(IdentityError::AccessDenied) => {
                    return Ok(protocol_error(
                        redirect,
                        Some(request.state),
                        "access_denied",
                    ));
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        let consent = consent_current(transaction, &target, &request, root.as_ref()).await?;
        let decision = interaction_decision(
            request.prompt,
            request.max_age,
            root.as_ref().map(|r| r.authenticated_at / 1000),
            now(self.service.clock.as_ref())? / 1000,
            flow.as_ref()
                .is_some_and(|f| f.reauth_completed_at.is_some()),
            consent,
        )?;
        match &self.input {
            Input::Start(_) => {
                match decision {
                    InteractionDecision::LoginRequired => {
                        return Ok(protocol_error(
                            redirect,
                            Some(request.state),
                            "login_required",
                        ));
                    }
                    InteractionDecision::ConsentRequired => {
                        return Ok(protocol_error(
                            redirect,
                            Some(request.state),
                            "consent_required",
                        ));
                    }
                    _ => (),
                }
                let proof = IssuedSecret::from_entropy(self.proof_entropy);
                let handle = FlowHandle::pending(
                    self.flow_id,
                    FlowPurpose::Authorization,
                    proof.digest(),
                    now(self.service.clock.as_ref())?
                        + self.service.config.identity.flow_ttl_seconds * 1000,
                )?;
                transaction
                    .apply(IdentityWrite::CreateFlow {
                        flow: handle.clone(),
                        interactive: decision != InteractionDecision::Continue,
                        environment_id: target.environment_id,
                        config_version_id: target.config_version_id,
                        browser_binding_digest: self.browser.clone(),
                        request_payload: parameters.to_payload(),
                    })
                    .await?;
                if let Some(root) = &root {
                    transaction
                        .apply(IdentityWrite::BindFlow {
                            flow_id: self.flow_id,
                            sso_session_id: root.id,
                            reauthenticated: false,
                        })
                        .await?;
                }
                match decision {
                    InteractionDecision::Login => Ok(AuthorizationResult {
                        outcome: AuthorizationOutcome::RequireLogin(handle),
                        flow_proof: Some(proof),
                    }),
                    InteractionDecision::Consent => Ok(AuthorizationResult {
                        outcome: AuthorizationOutcome::RequireConsent(
                            handle,
                            ConsentSummary {
                                application_name: target.application_name,
                                scopes: request.scopes,
                            },
                        ),
                        flow_proof: Some(proof),
                    }),
                    InteractionDecision::Continue => {
                        self.issue(
                            transaction,
                            self.flow_id,
                            admission.ok_or(IdentityError::Unauthenticated)?,
                            root.as_ref().ok_or(IdentityError::Unauthenticated)?,
                            parameters,
                            redirect,
                        )
                        .await
                    }
                    _ => Err(IdentityError::InvalidInput),
                }
            }
            Input::Complete { approved, .. } => {
                let flow = flow.ok_or(IdentityError::NotFound)?;
                let root = root.as_ref().ok_or(IdentityError::Unauthenticated)?;
                if !approved {
                    transaction
                        .apply(IdentityWrite::CompleteFlow {
                            flow_id: flow.handle.id(),
                        })
                        .await?;
                    self.audit(
                        transaction,
                        root.user_id,
                        "authorization.deny",
                        flow.handle.id(),
                        None,
                    )
                    .await?;
                    return Ok(protocol_error(
                        redirect,
                        Some(request.state),
                        "access_denied",
                    ));
                }
                if matches!(
                    decision,
                    InteractionDecision::Login | InteractionDecision::LoginRequired
                ) {
                    return Err(IdentityError::LoginRequired);
                }
                if decision == InteractionDecision::Consent {
                    transaction
                        .apply(IdentityWrite::SaveConsent {
                            user_id: root.user_id,
                            environment_id: target.environment_id,
                            config_version_id: target.config_version_id,
                            scopes: request.scopes.clone(),
                        })
                        .await?;
                    self.audit(
                        transaction,
                        root.user_id,
                        "consent.grant",
                        target.environment_id,
                        Some(request.scopes.as_protocol_string()),
                    )
                    .await?;
                }
                transaction
                    .apply(IdentityWrite::BindFlow {
                        flow_id: flow.handle.id(),
                        sso_session_id: root.id,
                        reauthenticated: false,
                    })
                    .await?;
                self.issue(
                    transaction,
                    flow.handle.id(),
                    admission.ok_or(IdentityError::Unauthenticated)?,
                    root,
                    parameters,
                    redirect,
                )
                .await
            }
        }
    }
}
impl AuthorizationUnit<'_> {
    async fn issue(
        &self,
        transaction: &mut dyn IdentityTransaction,
        flow_id: Uuid,
        admission: crate::domain::identity::admission::ManagedAdmission,
        root: &BrowserSessionSnapshot,
        parameters: AuthorizationParameters,
        redirect: ExactRedirectUri,
    ) -> Result<AuthorizationResult, IdentityError> {
        let request = parameters.validate()?;
        let code = IssuedSecret::from_entropy(self.code_entropy);
        transaction
            .apply(IdentityWrite::EnsureAccessGeneration {
                environment_id: admission.environment_id(),
                user_id: root.user_id,
            })
            .await?;

        transaction
            .apply(IdentityWrite::CreateRelyingParty {
                id: self.rp_id,
                protocol_sid: self.protocol_sid,
                admission: admission.clone(),
                nonce: Some(request.nonce),
            })
            .await?;
        transaction
            .apply(IdentityWrite::CreateCode {
                digest: code.digest(),
                flow_id,
                rp_session_id: self.rp_id,
                admission,
                verified_request: parameters.to_payload(),
                expires_at: (now(self.service.clock.as_ref())?
                    + self.service.config.identity.code_ttl_seconds * 1000)
                    .min(root.expires_at),
            })
            .await?;
        transaction
            .apply(IdentityWrite::CompleteFlow { flow_id })
            .await?;
        self.audit(
            transaction,
            root.user_id,
            "authorization.issue",
            self.rp_id,
            Some(request.scopes.as_protocol_string()),
        )
        .await?;
        Ok(AuthorizationResult {
            outcome: AuthorizationOutcome::RedirectToClient(ValidatedAuthorizationResponse {
                redirect_uri: redirect,
                code,
                state: request.state,
            }),
            flow_proof: None,
        })
    }
    async fn audit(
        &self,
        transaction: &mut dyn IdentityTransaction,
        user_id: i32,
        action: &'static str,
        object_id: Uuid,
        scope: Option<String>,
    ) -> Result<(), IdentityError> {
        let id = Uuid::new_v4();
        transaction
            .append_audit(&AuditEvent {
                id,
                request_id: self.request_id,
                actor_id: Some(user_id),
                action,
                object_kind: "authorization",
                object_id: object_id.to_string(),
                result: AuditResult::Success,
                reason: None,
                change_summary: serde_json::json!({"scope":scope}),
                operation_id: None,
                commit_marker: Some(id),
            })
            .await
    }
}
fn protocol_error(
    redirect_uri: ExactRedirectUri,
    state: Option<String>,
    error: &'static str,
) -> AuthorizationResult {
    AuthorizationResult {
        outcome: AuthorizationOutcome::ProtocolError(ValidatedAuthorizationError {
            redirect_uri,
            state,
            error,
        }),
        flow_proof: None,
    }
}
fn trusted_redirect(
    target: &AuthorizationTarget,
    parameters: &AuthorizationParameters,
) -> Result<ExactRedirectUri, IdentityError> {
    target
        .config
        .redirect_uris
        .iter()
        .find(|uri| {
            parameters
                .redirect_uri()
                .is_ok_and(|value| uri.matches(value))
        })
        .cloned()
        .ok_or(IdentityError::InvalidInput)
}
fn verify_version(
    flow: &AuthorizationFlowSnapshot,
    target: &AuthorizationTarget,
) -> Result<(), IdentityError> {
    if flow.environment_id != target.environment_id
        || flow.config_version_id != target.config_version_id
    {
        return Err(IdentityError::AccessDenied);
    }
    Ok(())
}
fn verify_flow(
    flow: &AuthorizationFlowSnapshot,
    proof: &AuthorizationFlowProof<'_>,
    browser: &SecretDigest,
    at: u64,
) -> Result<(), IdentityError> {
    flow.handle
        .verify(FlowPurpose::Authorization, proof.proof.expose_secret(), at)?;
    if &flow.browser_binding_digest != browser {
        return Err(IdentityError::Forbidden);
    }
    Ok(())
}
async fn load_root(
    transaction: &mut dyn IdentityTransaction,
    digest: Option<&SecretDigest>,
    issuer: &str,
    at: u64,
) -> Result<Option<BrowserSessionSnapshot>, IdentityError> {
    let Some(digest) = digest else {
        return Ok(None);
    };
    let Some(root) = transaction.load_browser_session(digest).await? else {
        return Ok(None);
    };
    if root.principal.issuer() != issuer {
        return Err(IdentityError::Unauthenticated);
    }
    let Some(account) = transaction.load_account(root.user_id).await? else {
        return Ok(None);
    };
    match root.validate(&account, at) {
        Ok(()) => Ok(Some(root)),
        Err(IdentityError::Unauthenticated) => Ok(None),
        Err(error) => Err(error),
    }
}
async fn consent_current(
    transaction: &mut dyn IdentityTransaction,
    target: &AuthorizationTarget,
    request: &AuthorizationRequest,
    root: Option<&BrowserSessionSnapshot>,
) -> Result<bool, IdentityError> {
    if target.config.consent_policy == ConsentPolicy::EnterprisePreconsent {
        return Ok(true);
    }
    let Some(root) = root else { return Ok(false) };
    Ok(transaction
        .current_consent(
            root.user_id,
            target.environment_id,
            target.config_version_id,
        )
        .await?
        .is_some_and(|scopes| request.scopes.is_subset_of(&scopes)))
}
