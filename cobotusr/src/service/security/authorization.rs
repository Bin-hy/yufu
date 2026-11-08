use super::{
    lifecycle::status_for_snapshot,
    repository::{SecurityRepository, now},
};
use auth::security::{
    budget::{Clock, SecurityBudget},
    credential::{AuthenticatedContext, CredentialRef, DecisionValidity},
    digest::RequestDigest,
    error::SecurityError,
    identity::{IdentityBinding, PrincipalKind, TokenUse},
    policy::{OperationContext, PolicyEffect, ResourceTarget, evaluate_policy},
};
use std::sync::Arc;

#[derive(Debug)]
pub struct AuthorizationDecision {
    pub decision_id: uuid::Uuid,
    pub effect: PolicyEffect,
    pub validity: DecisionValidity,
    pub credential_ref: CredentialRef,
    pub operation: OperationContext,
    pub bounds_fingerprint: RequestDigest,
}
pub struct AuthorizationService {
    repository: Arc<dyn SecurityRepository>,
    clock: Arc<dyn Clock>,
    management_application: String,
    management_audience: String,
}
enum PolicyRequest<'a> {
    Uri { method: &'a str, route: &'a str },
    Application(&'a OperationContext),
}
impl AuthorizationService {
    pub fn new(
        repository: Arc<dyn SecurityRepository>,
        clock: Arc<dyn Clock>,
        management_application: impl Into<String>,
        management_audience: impl Into<String>,
    ) -> Result<Self, SecurityError> {
        let management_application = management_application.into();
        let management_audience = management_audience.into();
        auth::security::policy::ResourceRef::new(
            &management_application,
            auth::security::policy::ResourceTarget::Application,
        )?;
        if management_audience.trim().is_empty()
            || management_audience.len() > 512
            || management_audience.chars().any(char::is_control)
        {
            return Err(SecurityError::InvalidResource);
        }
        Ok(Self {
            repository,
            clock,
            management_application,
            management_audience,
        })
    }
    pub async fn authorize_uri(
        &self,
        context: &AuthenticatedContext,
        method: &str,
        route: &str,
        budget: &SecurityBudget,
    ) -> Result<AuthorizationDecision, SecurityError> {
        if context.principal().kind() != PrincipalKind::User
            || context.token_use() != TokenUse::UserSession
            || context.application() != self.management_application
            || context.audience() != self.management_audience
        {
            return Err(SecurityError::Forbidden);
        }
        self.authorize(context, PolicyRequest::Uri { method, route }, budget)
            .await
    }
    /// Resource facts come from the trusted resource service, never from a user
    /// request's arbitrary subject/owner fields. T29 supplies the machine boundary.
    pub async fn authorize_application(
        &self,
        context: &AuthenticatedContext,
        operation: &OperationContext,
        budget: &SecurityBudget,
    ) -> Result<AuthorizationDecision, SecurityError> {
        if !matches!(
            context.token_use(),
            TokenUse::UserSession | TokenUse::McpAccess | TokenUse::SecurityService
        ) || context.application() != operation.resource.application()
        {
            return Err(SecurityError::Forbidden);
        }
        if let IdentityBinding::Gateway { gateway_id } = context.binding() {
            let gateway = match operation.resource.target() {
                ResourceTarget::Gateway { gateway_id }
                | ResourceTarget::Device { gateway_id, .. }
                | ResourceTarget::Tool { gateway_id, .. }
                | ResourceTarget::Task { gateway_id, .. }
                | ResourceTarget::GatewayKey { gateway_id, .. } => Some(gateway_id),
                _ => None,
            };
            if gateway != Some(gateway_id) {
                return Err(SecurityError::Forbidden);
            }
        }
        self.authorize(context, PolicyRequest::Application(operation), budget)
            .await
    }
    async fn authorize(
        &self,
        context: &AuthenticatedContext,
        request: PolicyRequest<'_>,
        budget: &SecurityBudget,
    ) -> Result<AuthorizationDecision, SecurityError> {
        context.ensure_current()?;
        // The caller can tighten this budget, but cannot restart authentication's
        // original request deadline by handing authorization a new 2-second budget.
        let budget = context.budget().child(budget.remaining()?)?;
        let mut transaction = self.repository.begin(&budget).await?;
        let registered = transaction
            .credential(context.credential())
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        let (status, key) = status_for_snapshot(&registered, now(self.clock.as_ref())?)?;
        if &status.claims != context.verified().claims()
            || key != context.verified().signing_key_id()
        {
            return Err(SecurityError::InvalidCredential);
        }
        let (snapshot, operation) = match request {
            PolicyRequest::Uri { method, route } => {
                transaction
                    .uri_policy(&status.claims, method, route)
                    .await?
            }
            PolicyRequest::Application(operation) => (
                transaction
                    .application_policy(&status.claims, operation)
                    .await?,
                operation.clone(),
            ),
        };
        let effect = evaluate_policy(&snapshot, &operation);
        let validity = DecisionValidity::from_query_start(
            status.validity.checked_at(),
            status.validity.valid_until(),
            None,
            Some(&context.validity()),
        )?;
        let bytes = serde_json::to_vec(context.bounds())
            .map_err(|_| SecurityError::DependencyUnavailable)?;
        transaction.commit().await?;
        budget.remaining()?;
        context.ensure_current()?;
        validity.ensure_fresh(self.clock.as_ref())?;
        Ok(AuthorizationDecision {
            decision_id: uuid::Uuid::new_v4(),
            effect,
            validity,
            credential_ref: context.credential().clone(),
            operation,
            bounds_fingerprint: RequestDigest::of(&bytes),
        })
    }
}
