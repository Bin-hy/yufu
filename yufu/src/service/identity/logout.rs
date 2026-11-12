//! RP initiated logout confirms one browser root; hints grant no authority.
use super::{
    account::{authenticate_current_actor, read_current_actor},
    browser::BrowserWriteEvidence,
    repository::*,
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::{
    config::Config,
    domain::identity::{
        account::{IssuedSecret, SecretDigest},
        authorization::strict_form_with_value_limit,
        error::IdentityError,
        session::{
            ActionFlowSnapshot, ActionTarget, DEFAULT_FLOW_TTL_SECONDS, IdentityRevocationScope,
        },
    },
    service::security::repository::now,
};
use auth::{
    security::{
        budget::{Clock, SecurityBudget},
        identity::SecretToken,
    },
    token_manager::IdTokenSigner,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Default)]
pub struct LogoutRequest {
    pub client_id: Option<String>,
    pub id_token_hint: Option<String>,
    pub post_logout_redirect_uri: Option<String>,
    pub state: Option<String>,
}
impl LogoutRequest {
    pub fn parse(raw: &str) -> Result<Self, IdentityError> {
        if raw.is_empty() {
            return Ok(Self::default());
        }
        let mut fields = strict_form_with_value_limit(raw, 16_384, 8192)?;
        if fields.iter().any(|(key, value)| {
            !matches!(
                key.as_str(),
                "client_id"
                    | "id_token_hint"
                    | "post_logout_redirect_uri"
                    | "state"
                    | "ui_locales"
                    | "logout_hint"
            ) || (key != "id_token_hint" && value.len() > 2048)
        }) {
            return Err(IdentityError::InvalidInput);
        }
        let request = Self {
            client_id: fields.remove("client_id"),
            id_token_hint: fields.remove("id_token_hint"),
            post_logout_redirect_uri: fields.remove("post_logout_redirect_uri"),
            state: fields.remove("state"),
        };
        request.validate()?;
        Ok(request)
    }
    fn validate(&self) -> Result<(), IdentityError> {
        if [&self.client_id, &self.post_logout_redirect_uri]
            .into_iter()
            .flatten()
            .any(|v| v.is_empty() || v.len() > 2048 || v.chars().any(char::is_control))
            || self
                .state
                .as_ref()
                .is_some_and(|v| v.len() > 2048 || v.chars().any(char::is_control))
            || self
                .id_token_hint
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.len() > 8192)
        {
            return Err(IdentityError::InvalidInput);
        }
        Ok(())
    }
}

pub enum LogoutStart {
    Confirmation {
        flow: ActionFlowSnapshot,
        proof: IssuedSecret,
        application_name: Option<String>,
    },
    Complete(LogoutResult),
}
pub struct LogoutResult {
    pub redirect: Option<String>,
}

pub struct LogoutService {
    repository: Arc<dyn IdentityRepository>,
    clock: Arc<dyn Clock>,
    signer: Arc<IdTokenSigner>,
    config: Config,
}
impl LogoutService {
    pub fn new(
        repository: Arc<dyn IdentityRepository>,
        clock: Arc<dyn Clock>,
        signer: Arc<IdTokenSigner>,
        config: Config,
    ) -> Self {
        Self {
            repository,
            clock,
            signer,
            config,
        }
    }
    pub async fn start(
        &self,
        center_secret: Option<&str>,
        browser_binding_digest: SecretDigest,
        request: LogoutRequest,
        budget: &SecurityBudget,
    ) -> Result<LogoutStart, IdentityError> {
        request.validate()?;
        if let Some(secret) = center_secret {
            super::sso::validate_opaque(secret)?;
        }
        let reference = request
            .id_token_hint
            .as_ref()
            .map(|hint| {
                self.signer
                    .logout_reference(
                        &SecretToken::new(hint.clone())?,
                        &self.config.issuer(),
                        now(self.clock.as_ref())? / 1000,
                    )
                    .map_err(IdentityError::from)
            })
            .transpose()?;
        if reference.as_ref().is_some_and(|r| {
            request
                .client_id
                .as_ref()
                .is_some_and(|id| id != &r.client_id)
        }) {
            return Err(IdentityError::InvalidInput);
        }
        execute_identity(
            self.repository.as_ref(),
            budget,
            &Start {
                service: self,
                center: center_secret.map(SecretDigest::of),
                binding: browser_binding_digest,
                request,
                reference,
                flow_id: Uuid::new_v4(),
                proof_entropy: rand::random(),
            },
        )
        .await
    }
    pub async fn confirm(
        &self,
        evidence: &BrowserWriteEvidence,
        flow_id: Uuid,
        proof: &str,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<LogoutResult, IdentityError> {
        if flow_id.is_nil() {
            return Err(IdentityError::InvalidInput);
        }
        super::sso::validate_opaque(proof)?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &Confirm {
                service: self,
                evidence,
                flow_id,
                proof: SecretDigest::of(proof),
                request_id,
            },
        )
        .await
    }
}

fn redirect(uri: Option<&str>, state: Option<&str>) -> Result<Option<String>, IdentityError> {
    uri.map(|uri| {
        // Keep the registered URI byte-for-byte, appending only the encoded state.
        url::Url::parse(uri).map_err(|_| IdentityError::InvalidInput)?;
        match state {
            None => Ok(uri.to_owned()),
            Some(state) => {
                let query = url::form_urlencoded::Serializer::new(String::new())
                    .append_pair("state", state)
                    .finish();
                Ok(format!(
                    "{uri}{}{query}",
                    if uri.contains('?') {
                        if uri.ends_with('?') || uri.ends_with('&') {
                            ""
                        } else {
                            "&"
                        }
                    } else {
                        "?"
                    }
                ))
            }
        }
    })
    .transpose()
}
fn registered_redirect(
    target: Option<&AuthorizationTarget>,
    uri: Option<&str>,
) -> Result<(), IdentityError> {
    if let Some(uri) = uri
        && target.is_none_or(|target| {
            !target
                .config
                .post_logout_redirect_uris
                .iter()
                .any(|u| u.as_str() == uri)
        })
    {
        return Err(IdentityError::InvalidInput);
    }
    Ok(())
}
struct Start<'a> {
    service: &'a LogoutService,
    center: Option<SecretDigest>,
    binding: SecretDigest,
    request: LogoutRequest,
    reference: Option<auth::claims::LogoutReference>,
    flow_id: Uuid,
    proof_entropy: [u8; 32],
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for Start<'_> {
    type Output = LogoutStart;
    async fn apply(&self, tx: &mut dyn IdentityTransaction) -> Result<LogoutStart, IdentityError> {
        let at = now(self.service.clock.as_ref())?;
        let client_id = self
            .reference
            .as_ref()
            .map(|r| r.client_id.as_str())
            .or(self.request.client_id.as_deref());
        let initial_target = match client_id {
            Some(id) => Some(tx.authorization_target(id, None).await?),
            None => None,
        };
        registered_redirect(
            initial_target.as_ref(),
            self.request.post_logout_redirect_uri.as_deref(),
        )?;
        let rp = match &self.reference {
            Some(reference) => {
                let rp = tx
                    .logout_rp_reference(reference)
                    .await?
                    .ok_or(IdentityError::Forbidden)?;
                if rp
                    .revoked_at
                    .is_some_and(|revoked| at.saturating_sub(revoked as u64) > 86_400_000)
                {
                    return Err(IdentityError::Forbidden);
                }
                Some(rp)
            }
            None => None,
        };
        let complete = || {
            redirect(
                self.request.post_logout_redirect_uri.as_deref(),
                self.request.state.as_deref(),
            )
            .map(|redirect| LogoutStart::Complete(LogoutResult { redirect }))
        };
        let Some(digest) = &self.center else {
            return complete();
        };
        let initial_actor = match read_current_actor(tx, digest, at).await {
            Ok(actor) => actor,
            Err(IdentityError::Unauthenticated) => return complete(),
            Err(e) => return Err(e),
        };
        if rp
            .as_ref()
            .is_some_and(|rp| rp.sso_session_id != initial_actor.sso_session_id())
        {
            return Err(IdentityError::Forbidden);
        }
        let mut keys = initial_target
            .as_ref()
            .map(|t| t.locks.keys().to_vec())
            .unwrap_or_default();
        if let Some(rp) = &rp {
            keys.push(IdentityLockKey::RelyingParty(rp.id));
        }
        let actor = authenticate_current_actor(
            tx,
            digest,
            now(self.service.clock.as_ref())?,
            &IdentityLockScope::new(keys)?,
        )
        .await?;
        if actor.sso_session_id() != initial_actor.sso_session_id() {
            return Err(IdentityError::Conflict);
        }
        if let Some(reference) = &self.reference {
            let current = tx
                .logout_rp_reference(reference)
                .await?
                .ok_or(IdentityError::Forbidden)?;
            if current.sso_session_id != actor.sso_session_id() {
                return Err(IdentityError::Forbidden);
            }
        }
        let current_target = match client_id {
            Some(id) => Some(tx.authorization_target(id, None).await?),
            None => None,
        };
        if let (Some(before), Some(after)) = (&initial_target, &current_target)
            && (before.locks.keys() != after.locks.keys()
                || before.config_version_id != after.config_version_id)
        {
            return Err(IdentityError::Conflict);
        }
        registered_redirect(
            current_target.as_ref(),
            self.request.post_logout_redirect_uri.as_deref(),
        )?;
        let root = tx
            .load_browser_session(digest)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        let proof = IssuedSecret::from_entropy(self.proof_entropy);
        let mut payload = serde_json::Map::new();
        for (key, value) in [
            ("client_id", client_id.map(str::to_owned)),
            (
                "config_version_id",
                current_target
                    .as_ref()
                    .map(|t| t.config_version_id.to_string()),
            ),
            ("rp_session_id", rp.map(|rp| rp.id.to_string())),
            (
                "post_logout_redirect_uri",
                self.request.post_logout_redirect_uri.clone(),
            ),
            ("state", self.request.state.clone()),
        ] {
            if let Some(value) = value {
                payload.insert(key.into(), value.into());
            }
        }
        let flow = ActionFlowSnapshot {
            id: self.flow_id,
            target: ActionTarget::LogoutConfirmation(root.id),
            browser_binding_digest: self.binding.clone(),
            browser_proof_digest: proof.digest(),
            expires_at: root
                .expires_at
                .min(at.saturating_add(DEFAULT_FLOW_TTL_SECONDS * 1000)),
            consumed_at: None,
            request_payload: payload.into(),
        };
        tx.apply(IdentityWrite::CreateActionFlow(flow.clone()))
            .await?;
        Ok(LogoutStart::Confirmation {
            flow,
            proof,
            application_name: current_target.map(|t| t.application_name),
        })
    }
}
struct Confirm<'a> {
    service: &'a LogoutService,
    evidence: &'a BrowserWriteEvidence,
    flow_id: Uuid,
    proof: SecretDigest,
    request_id: Uuid,
}
#[async_trait::async_trait]
impl IdentityUnitOfWork for Confirm<'_> {
    type Output = LogoutResult;
    async fn apply(&self, tx: &mut dyn IdentityTransaction) -> Result<LogoutResult, IdentityError> {
        let digest = self
            .evidence
            .current_center_digest
            .as_ref()
            .ok_or(IdentityError::Unauthenticated)?;
        let initial = tx
            .load_action_flow(self.flow_id)
            .await?
            .ok_or(IdentityError::NotFound)?;
        let ActionTarget::LogoutConfirmation(root_id) = initial.target else {
            return Err(IdentityError::Forbidden);
        };
        initial.verify(
            initial.target,
            &self.evidence.browser_binding_digest,
            &self.proof,
            now(self.service.clock.as_ref())?,
        )?;
        let scope = IdentityRevocationScope::CurrentCenter {
            sso_session_id: root_id,
        };
        let plan = tx.revocation_plan(&scope).await?;
        let client = initial
            .request_payload
            .get("client_id")
            .and_then(|v| v.as_str());
        let target = match client {
            Some(id) => Some(tx.authorization_target(id, None).await?),
            None => None,
        };
        let mut keys = plan.locks.keys().to_vec();
        keys.push(IdentityLockKey::Action(self.flow_id));
        if let Some(target) = &target {
            keys.extend_from_slice(target.locks.keys());
        }
        let actor = authenticate_current_actor(
            tx,
            digest,
            now(self.service.clock.as_ref())?,
            &IdentityLockScope::new(keys)?,
        )
        .await?;
        // Ownership of the same account in another browser is insufficient.
        if actor.sso_session_id() != root_id {
            return Err(IdentityError::Forbidden);
        }
        let flow = tx
            .load_action_flow(self.flow_id)
            .await?
            .ok_or(IdentityError::NotFound)?;
        flow.verify(
            ActionTarget::LogoutConfirmation(root_id),
            &self.evidence.browser_binding_digest,
            &self.proof,
            now(self.service.clock.as_ref())?,
        )?;
        let current_target = match client {
            Some(id) => Some(tx.authorization_target(id, None).await?),
            None => None,
        };
        if let (Some(before), Some(after)) = (&target, &current_target)
            && (before.locks.keys() != after.locks.keys()
                || flow
                    .request_payload
                    .get("config_version_id")
                    .and_then(|v| v.as_str())
                    != Some(after.config_version_id.to_string().as_str()))
        {
            return Err(IdentityError::Conflict);
        }
        let current = tx.revocation_plan(&scope).await?;
        if current.locks.keys() != plan.locks.keys()
            || current.rp_session_ids != plan.rp_session_ids
        {
            return Err(IdentityError::Conflict);
        }
        let uri = flow
            .request_payload
            .get("post_logout_redirect_uri")
            .and_then(|v| v.as_str());
        registered_redirect(current_target.as_ref(), uri)?;
        let redirect = redirect(
            uri,
            flow.request_payload.get("state").and_then(|v| v.as_str()),
        )?;
        let live = tx.live_rp_ids(&current.rp_session_ids).await?;
        tx.apply(IdentityWrite::ConsumeActionFlow {
            id: self.flow_id,
            target: flow.target,
            browser_binding_digest: self.evidence.browser_binding_digest.clone(),
            browser_proof_digest: self.proof.clone(),
        })
        .await?;
        tx.apply(IdentityWrite::Revoke(scope)).await?;
        let remaining = tx.live_rp_ids(&live).await?;
        let revoked: Vec<_> = live
            .into_iter()
            .filter(|id| !remaining.contains(id))
            .collect();
        let event_id = Uuid::new_v4();
        tx.append_audit(&AuditEvent {
            id: event_id,
            request_id: self.request_id,
            actor_id: Some(actor.account().user_id),
            action: "session.rp_logout",
            object_kind: "session",
            object_id: root_id.to_string(),
            result: AuditResult::Success,
            reason: None,
            change_summary: serde_json::json!({"affected_count":revoked.len()}),
            operation_id: None,
            commit_marker: Some(event_id),
        })
        .await?;
        tx.enqueue_logout(&LogoutEvent {
            audit_event_id: event_id,
            rp_session_ids: revoked,
        })
        .await?;
        Ok(LogoutResult { redirect })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logout_input_is_strict_and_state_append_preserves_registered_query() {
        for raw in [
            "state=x&st%61te=y",
            "state=%Q1",
            "client_id=",
            "state=%00",
            "return_to=https%3A%2F%2Fevil.test",
        ] {
            assert_eq!(
                LogoutRequest::parse(raw).err(),
                Some(IdentityError::InvalidInput)
            );
        }
        assert_eq!(
            redirect(Some("https://rp.test/end?original=%2f"), Some("a +&")).unwrap(),
            Some("https://rp.test/end?original=%2f&state=a+%2B%26".into())
        );
        assert_eq!(redirect(None, Some("state")).unwrap(), None);
        assert!(LogoutRequest::parse("").is_ok());
    }
}
