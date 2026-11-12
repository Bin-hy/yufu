use super::*;
use crate::domain::identity::introspection::Introspection;
use auth::{
    security::{credential::CredentialRef, error::SecurityError, identity::SecretToken},
    token_manager::IdTokenSigner,
};

pub struct ProtocolClientAuthentication<'a> {
    pub id: &'a str,
    pub secret: Option<&'a SecretToken>,
}
struct QueryBinding {
    client_id: String,
    environment_id: Option<Uuid>,
    audience: Option<String>,
}

fn authentication_error(error: SecurityError) -> IdentityError {
    match error {
        SecurityError::InvalidCredential | SecurityError::CredentialRevoked => {
            IdentityError::Unauthenticated
        }
        error => error.into(),
    }
}
async fn authenticate(
    tx: &mut dyn IdentityTransaction,
    auth: &ProtocolClientAuthentication<'_>,
    allow_reader: bool,
    allow_public: bool,
) -> Result<QueryBinding, IdentityError> {
    if let Some(client) = tx.oidc_client(auth.id).await? {
        if client.public_client && !allow_public {
            return Err(IdentityError::Unauthenticated);
        }
        authenticate_session_client(&client, auth.secret, "")
            .await
            .map_err(authentication_error)?;
        tx.renew_budget()?;
        return Ok(QueryBinding {
            client_id: client.client_id,
            environment_id: client.environment_id,
            audience: None,
        });
    }
    if !allow_reader {
        return Err(IdentityError::Unauthenticated);
    }
    let reader = tx
        .introspection_reader(auth.id)
        .await?
        .filter(|r| r.enabled)
        .ok_or(IdentityError::Unauthenticated)?;
    let plaintext = auth
        .secret
        .filter(|s| s.expose_secret().len() <= 72)
        .ok_or(IdentityError::Unauthenticated)?
        .expose_secret()
        .to_owned();
    let hash = reader.secret_hash;
    let valid = crate::service::identity::password::verify(plaintext, hash).await?;
    if !valid {
        return Err(IdentityError::Unauthenticated);
    }
    tx.renew_budget()?;
    let client = tx
        .environment_client(reader.environment_id)
        .await?
        .ok_or(IdentityError::Unauthenticated)?;
    let target = match tx.authorization_target(&client.client_id, None).await {
        Ok(target) => target,
        Err(IdentityError::AccessDenied) => return Err(IdentityError::Unauthenticated),
        Err(error) => return Err(error),
    };
    if !target.enabled
        || client.management_state != "managed"
        || !target.config.client_kind.permits_introspection_reader()
        || target.config.access_audience.as_str() != reader.approved_audience
    {
        return Err(IdentityError::Unauthenticated);
    }
    Ok(QueryBinding {
        client_id: client.client_id,
        environment_id: Some(reader.environment_id),
        audience: Some(reader.approved_audience),
    })
}

impl ProtocolService {
    pub async fn introspect(
        &self,
        auth: ProtocolClientAuthentication<'_>,
        token: &SecretToken,
        verifier: &IdTokenSigner,
        budget: &SecurityBudget,
    ) -> Result<Introspection, IdentityError> {
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let binding = authenticate(tx.as_mut(), &auth, true, false).await?;
            let access = match self
                .lifecycle
                .identity_protocol_token_in_transaction(
                    tx.as_mut(),
                    token,
                    &self.config.issuer(),
                    verifier,
                )
                .await
            {
                Ok(access) => access,
                Err(SecurityError::InvalidCredential | SecurityError::CredentialRevoked) => {
                    return Ok((Introspection::inactive(), None));
                }
                Err(error) => return Err(error.into()),
            };
            if access.client_id != binding.client_id
                || binding
                    .audience
                    .as_ref()
                    .is_some_and(|a| a != &access.audience)
                || access
                    .context
                    .as_ref()
                    .is_some_and(|c| Some(c.admission.environment_id()) != binding.environment_id)
            {
                return Ok((Introspection::inactive(), None));
            }
            let scope = if let Some(context) = &access.context {
                let user_id = access
                    .subject
                    .parse()
                    .map_err(|_| IdentityError::AccessDenied)?;
                let target = match tx
                    .authorization_target(&binding.client_id, Some(user_id))
                    .await
                {
                    Ok(target) => target,
                    Err(IdentityError::AccessDenied) => {
                        return Ok((Introspection::inactive(), None));
                    }
                    Err(error) => return Err(error),
                };
                if !target.enabled
                    || target.config_version_id != context.admission.config_version_id()
                {
                    return Ok((Introspection::inactive(), None));
                }
                if target.config.consent_policy != ConsentPolicy::EnterprisePreconsent {
                    let consent = tx
                        .current_consent(user_id, target.environment_id, target.config_version_id)
                        .await?;
                    if !consent
                        .is_some_and(|scopes| context.admission.scopes().is_subset_of(&scopes))
                    {
                        return Ok((Introspection::inactive(), None));
                    }
                }
                context.admission.scopes().as_protocol_string()
            } else {
                "openid".into()
            };
            Ok((
                Introspection {
                    active: true,
                    scope: Some(scope),
                    client_id: Some(access.client_id),
                    token_type: Some("Bearer"),
                    exp: Some(access.expires_at),
                    sub: Some(access.subject),
                    sid: access.context.map(|c| c.protocol_sid.to_string()),
                    aud: Some(access.audience),
                    iss: Some(self.config.issuer()),
                },
                Some(access.validity),
            ))
        }
        .await;
        match result {
            Ok((response, validity)) => {
                tx.commit().await?;
                budget.remaining()?;
                if let Some(validity) = validity {
                    validity.ensure_fresh(self.clock.as_ref())?;
                }
                Ok(response)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }
    pub async fn revoke(
        &self,
        auth: ProtocolClientAuthentication<'_>,
        token: &SecretToken,
        verifier: &IdTokenSigner,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<(), IdentityError> {
        let reference = if token.expose_secret().split('.').count() == 3 {
            match verifier.revocation_reference(token, &self.config.issuer()) {
                Ok(reference) => Some(reference),
                Err(SecurityError::InvalidCredential) => None,
                Err(error) => return Err(error.into()),
            }
        } else {
            None
        };
        execute_identity(
            self.repository.as_ref(),
            budget,
            &Revoke {
                service: self,
                auth,
                token,
                reference: reference.as_ref(),
                request_id,
            },
        )
        .await
    }
}
struct Revoke<'a> {
    service: &'a ProtocolService,
    auth: ProtocolClientAuthentication<'a>,
    token: &'a SecretToken,
    reference: Option<&'a CredentialRef>,
    request_id: Uuid,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for Revoke<'_> {
    type Output = ();
    async fn apply(&self, tx: &mut dyn IdentityTransaction) -> Result<(), IdentityError> {
        let issuer = self.service.config.issuer();
        let binding = authenticate(tx, &self.auth, false, true).await?;
        let Some(initial) = tx
            .protocol_revocation_target(&issuer, self.token, self.reference)
            .await?
        else {
            return Ok(());
        };
        if initial.client_id != binding.client_id
            || Some(initial.environment_id) != binding.environment_id
            || initial.revoked
        {
            return Ok(());
        }
        let scope = IdentityRevocationScope::Grant {
            grant_id: initial.grant_id,
        };
        let plan = tx.revocation_plan(&scope).await?;
        tx.lock_identity_scope(&plan.locks).await?;
        let current_binding = authenticate(tx, &self.auth, false, true).await?;
        let current = tx
            .protocol_revocation_target(&issuer, self.token, self.reference)
            .await?
            .ok_or(IdentityError::Conflict)?;
        let current_plan = tx.revocation_plan(&scope).await?;
        if current != initial
            || current_binding.client_id != initial.client_id
            || current_binding.environment_id != Some(initial.environment_id)
            || plan.locks.keys() != current_plan.locks.keys()
            || plan.rp_session_ids != current_plan.rp_session_ids
        {
            return Err(IdentityError::Conflict);
        }
        tx.apply(IdentityWrite::Revoke(scope)).await?;
        let id = Uuid::new_v4();
        tx.append_audit(&AuditEvent {
            id,request_id:self.request_id,actor_id:None,action:"token.revoke",
            object_kind:"grant",object_id:initial.grant_id.to_string(),result:AuditResult::Success,reason:None,
            change_summary:serde_json::json!({"client_id":initial.client_id,"target_user_id":initial.user_id,"affected_count":current_plan.rp_session_ids.len()}),
            operation_id:None,commit_marker:Some(id),
        }).await?;
        tx.enqueue_logout(&LogoutEvent {
            audit_event_id: id,
            rp_session_ids: current_plan.rp_session_ids,
        })
        .await?;
        Ok(())
    }
}
