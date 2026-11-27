//! External authentication orchestration. HTTP calls stay outside the 2s write budget.
use super::{
    BeginContext, BeginOutcome, CompleteContext, ExternalHttpClient, ExternalIdentityAdapter,
    ExternalIdentityRegistry, ExternalRateLimiter, SecretCipher,
};
use crate::config::Config;
use crate::domain::identity::{
    account::{IssuedSecret, SecretDigest, ensure_enabled, stable_principal},
    error::IdentityError,
    external::{
        ExternalFlowWrite, ExternalIdentity, ExternalIdentityRecord, ExternalIdentityWrite,
        ExternalProviderKey, ExternalPurpose, NewExternalAccountWrite, RegistrationPolicy,
    },
    session::{BrowserSessionSnapshot, IdentityRevocationScope},
};
use crate::service::identity::{
    account::{authenticate_current_actor, read_current_actor},
    repository::{
        AuditEvent, AuditResult, IdentityLockKey, IdentityLockScope, IdentityRepository,
        IdentityRevocationPlan, IdentityTransaction, IdentityWrite, LogoutEvent,
    },
    sso::validate_opaque,
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::service::security::repository::now;
use auth::security::budget::{Clock, SecurityBudget};
use serde::Serialize;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use uuid::Uuid;

const START_LIMIT: u32 = 20;
const COMPLETE_LIMIT: u32 = 20;
const WINDOW: Duration = Duration::from_secs(60);

pub struct ExternalIdentityService {
    repository: Arc<dyn IdentityRepository>,
    clock: Arc<dyn Clock>,
    config: Config,
    registry: Arc<RwLock<ExternalIdentityRegistry>>,
    cipher: Option<Arc<SecretCipher>>,
    http: Arc<ExternalHttpClient>,
    limiter: Arc<ExternalRateLimiter>,
}

pub struct StartedExternalLogin {
    pub authorize_url: String,
}

pub enum ExternalCompletion {
    Login {
        cookie_secret: IssuedSecret,
        expires_at: u64,
        oidc_flow_id: Option<Uuid>,
    },
    Bind {
        provider_key: ExternalProviderKey,
    },
    Rejected {
        purpose: ExternalPurpose,
        code: &'static str,
    },
}

#[derive(Serialize)]
pub struct ExternalIdentityView {
    pub id: Uuid,
    pub provider_key: ExternalProviderKey,
    pub provider_label: &'static str,
    pub display_name: Option<String>,
    pub subject_hint: String,
    pub created_at: u64,
    pub last_login_at: Option<u64>,
}

impl ExternalIdentityService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repository: Arc<dyn IdentityRepository>,
        clock: Arc<dyn Clock>,
        config: Config,
        registry: Arc<RwLock<ExternalIdentityRegistry>>,
        cipher: Option<Arc<SecretCipher>>,
        http: Arc<ExternalHttpClient>,
        limiter: Arc<ExternalRateLimiter>,
    ) -> Result<Self, IdentityError> {
        config
            .validate_identity_policy()
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        Ok(Self {
            repository,
            clock,
            config,
            registry,
            cipher,
            http,
            limiter,
        })
    }

    pub async fn start_login(
        &self,
        provider_key: ExternalProviderKey,
        flow_id: Option<Uuid>,
        browser: &SecretDigest,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<StartedExternalLogin, IdentityError> {
        self.start(
            ExternalPurpose::Login,
            provider_key,
            flow_id,
            None,
            browser,
            request_id,
            budget,
        )
        .await
    }

    pub async fn start_bind(
        &self,
        provider_key: ExternalProviderKey,
        user_secret: &str,
        browser: &SecretDigest,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<StartedExternalLogin, IdentityError> {
        validate_opaque(user_secret)?;
        self.start(
            ExternalPurpose::Bind,
            provider_key,
            None,
            Some(user_secret.to_owned()),
            browser,
            request_id,
            budget,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn start(
        &self,
        purpose: ExternalPurpose,
        provider_key: ExternalProviderKey,
        flow_id: Option<Uuid>,
        user_secret: Option<String>,
        browser: &SecretDigest,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<StartedExternalLogin, IdentityError> {
        self.ensure_enabled()?;
        self.limiter.check(
            &format!("{}:{:?}:{}", provider_key, purpose, browser.hex()),
            START_LIMIT,
            WINDOW,
        )?;
        let (adapter, provider_config) = self.adapter_and_config(provider_key)?;
        let state = IssuedSecret::from_entropy(rand::random());
        let state_digest = state.digest();
        let state_value = state.into_response();
        let callback_uri = self.callback_uri(provider_key);
        let outcome = adapter
            .begin(BeginContext {
                callback_uri: &callback_uri,
                state: &state_value,
                nonce: None,
                code_challenge: None,
                config: &provider_config,
            })
            .await?;
        let BeginOutcome::Redirect {
            authorize_url,
            secret_payload,
        } = outcome
        else {
            return Err(IdentityError::InvalidInput);
        };
        let secret_ciphertext = match (secret_payload, &self.cipher) {
            (Some(payload), Some(cipher)) => {
                Some(cipher.encrypt(provider_key.as_str(), payload.expose_secret())?)
            }
            (Some(_), None) => return Err(IdentityError::DependencyUnavailable),
            (None, _) => None,
        };
        let expires_at =
            now(self.clock.as_ref())? + self.config.external_identity.flow_ttl_seconds * 1000;
        let flow_write_id = Uuid::new_v4();
        execute_identity(
            self.repository.as_ref(),
            budget,
            &StartExternalFlow {
                service: self,
                purpose,
                provider_key,
                user_secret,
                state_digest,
                browser: browser.clone(),
                oidc_flow_id: flow_id,
                secret_ciphertext,
                expires_at,
                request_id,
                flow_write_id,
            },
        )
        .await?;
        Ok(StartedExternalLogin { authorize_url })
    }

    fn adapter_and_config(
        &self,
        key: ExternalProviderKey,
    ) -> Result<
        (
            Arc<dyn ExternalIdentityAdapter>,
            super::ExternalProviderConfig,
        ),
        IdentityError,
    > {
        let registry = self
            .registry
            .read()
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        Ok((
            registry.adapter(key)?.clone(),
            registry.config(key)?.clone(),
        ))
    }

    fn callback_uri(&self, key: ExternalProviderKey) -> String {
        format!(
            "{}/identity/external/{}/callback",
            self.config.issuer().trim_end_matches('/'),
            key.as_str()
        )
    }

    fn ensure_enabled(&self) -> Result<(), IdentityError> {
        if self.config.external_identity.enabled {
            Ok(())
        } else {
            Err(IdentityError::NotFound)
        }
    }
}

struct StartExternalFlow<'a> {
    service: &'a ExternalIdentityService,
    purpose: ExternalPurpose,
    provider_key: ExternalProviderKey,
    user_secret: Option<String>,
    state_digest: SecretDigest,
    browser: SecretDigest,
    oidc_flow_id: Option<Uuid>,
    secret_ciphertext: Option<String>,
    expires_at: u64,
    request_id: Uuid,
    flow_write_id: Uuid,
}

#[async_trait::async_trait]
impl IdentityUnitOfWork for StartExternalFlow<'_> {
    type Output = ();
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        let at = now(self.service.clock.as_ref())?;
        let user_id = match &self.user_secret {
            Some(secret) => {
                validate_opaque(secret)?;
                let actor = authenticate_current_actor(
                    transaction,
                    &SecretDigest::of(secret),
                    at,
                    &IdentityLockScope::new(Vec::new())?,
                )
                .await?;
                Some(actor.account().user_id)
            }
            None => None,
        };
        let flow = ExternalFlowWrite {
            id: self.flow_write_id,
            provider_key: self.provider_key,
            purpose: self.purpose,
            state_digest: self.state_digest.clone(),
            browser_binding_digest: self.browser.clone(),
            user_id,
            oidc_flow_id: self.oidc_flow_id,
            nonce: None,
            secret_ciphertext: self.secret_ciphertext.clone(),
            expires_at: self.expires_at,
        };
        flow.validate(at)?;
        transaction
            .apply(IdentityWrite::CreateExternalFlow(flow))
            .await?;
        append_audit(
            transaction,
            self.request_id,
            user_id,
            match self.purpose {
                ExternalPurpose::Login => "external.login.start",
                ExternalPurpose::Bind => "external.bind.start",
            },
            "external_flow",
            self.flow_write_id.to_string(),
            AuditResult::Success,
            None,
            serde_json::json!({"provider": self.provider_key.as_str()}),
        )
        .await
    }
}

impl ExternalIdentityService {
    #[allow(clippy::too_many_arguments)]
    pub async fn complete(
        &self,
        provider_key: ExternalProviderKey,
        code: &str,
        state: &str,
        browser: &SecretDigest,
        center_secret: Option<&str>,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<ExternalCompletion, IdentityError> {
        self.ensure_enabled()?;
        self.limiter.check(
            &format!("{}:complete:{}", provider_key, browser.hex()),
            COMPLETE_LIMIT,
            WINDOW,
        )?;
        let state_digest = SecretDigest::of(state);
        let flow = self
            .failure(
                provider_key,
                request_id,
                self.load_flow(&state_digest, budget).await,
            )
            .await?;
        if flow.provider_key != provider_key {
            let reason = IdentityError::Forbidden;
            self.record_failure(provider_key, reason, request_id).await;
            return Err(reason);
        }
        self.failure(
            provider_key,
            request_id,
            flow.validate(browser, now(self.clock.as_ref())?),
        )
        .await?;
        let verifier = match (&flow.secret_ciphertext, &self.cipher) {
            (Some(ciphertext), Some(cipher)) => Some(
                self.failure(
                    provider_key,
                    request_id,
                    cipher.decrypt(provider_key.as_str(), ciphertext),
                )
                .await?,
            ),
            (Some(_), None) => {
                let reason = IdentityError::DependencyUnavailable;
                self.record_failure(provider_key, reason, request_id).await;
                return Err(reason);
            }
            (None, _) => None,
        };
        let (adapter, provider_config) = match self.adapter_and_config(provider_key) {
            Ok(value) => value,
            Err(reason) => {
                self.record_failure(provider_key, reason, request_id).await;
                return Err(reason);
            }
        };
        let _permit = self.limiter.try_acquire_upstream()?;
        let identity = self
            .failure(
                provider_key,
                request_id,
                adapter
                    .complete(CompleteContext {
                        code,
                        callback_uri: &self.callback_uri(provider_key),
                        code_verifier: verifier.as_ref(),
                        nonce: flow.nonce.as_deref(),
                        config: &provider_config,
                        http: &self.http,
                    })
                    .await,
            )
            .await?;
        self.failure(provider_key, request_id, identity.validate())
            .await?;
        let center = match center_secret {
            Some(secret) => {
                validate_opaque(secret)?;
                Some(secret.to_owned())
            }
            None => None,
        };
        // The request budget is spent on state validation and the upstream call.
        let write_budget = SecurityBudget::new(self.clock.clone())?;
        execute_identity(
            self.repository.as_ref(),
            &write_budget,
            &CompleteExternal {
                service: self,
                provider_key,
                state_digest,
                browser: browser.clone(),
                center,
                identity,
                request_id,
            },
        )
        .await
    }

    async fn load_flow(
        &self,
        digest: &SecretDigest,
        budget: &SecurityBudget,
    ) -> Result<crate::domain::identity::external::ExternalFlowSnapshot, IdentityError> {
        let mut transaction = self.repository.begin(budget).await?;
        let result = transaction.external_flow_by_state_digest(digest).await;
        transaction.rollback().await?;
        result?.ok_or(IdentityError::NotFound)
    }

    async fn failure<T>(
        &self,
        provider_key: ExternalProviderKey,
        request_id: Uuid,
        result: Result<T, IdentityError>,
    ) -> Result<T, IdentityError> {
        match result {
            Ok(value) => Ok(value),
            Err(reason) => {
                self.record_failure(provider_key, reason, request_id).await;
                Err(reason)
            }
        }
    }

    async fn record_failure(
        &self,
        provider_key: ExternalProviderKey,
        reason: IdentityError,
        request_id: Uuid,
    ) {
        let Ok(budget) = SecurityBudget::new(self.clock.clone()) else {
            return;
        };
        if execute_identity(
            self.repository.as_ref(),
            &budget,
            &ExternalFailure {
                provider_key,
                reason,
                request_id,
            },
        )
        .await
        .is_err()
        {
            tracing::warn!(
                provider = provider_key.as_str(),
                reason = reason.code(),
                "external failure audit unavailable"
            );
        }
    }

    pub async fn list_identities(
        &self,
        user_secret: &str,
        budget: &SecurityBudget,
    ) -> Result<Vec<ExternalIdentityView>, IdentityError> {
        validate_opaque(user_secret)?;
        let mut transaction = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                transaction.as_mut(),
                &SecretDigest::of(user_secret),
                now(self.clock.as_ref())?,
            )
            .await?;
            let records = transaction
                .external_identities_for_user(actor.account().user_id)
                .await?;
            Ok(records.into_iter().map(view).collect())
        }
        .await;
        transaction.rollback().await?;
        result
    }

    pub async fn unbind(
        &self,
        user_secret: &str,
        identity_id: Uuid,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<(), IdentityError> {
        validate_opaque(user_secret)?;
        execute_identity(
            self.repository.as_ref(),
            budget,
            &UnbindExternalIdentity {
                service: self,
                user_secret: user_secret.to_owned(),
                identity_id,
                request_id,
            },
        )
        .await
    }
}

struct CompleteExternal<'a> {
    service: &'a ExternalIdentityService,
    provider_key: ExternalProviderKey,
    state_digest: SecretDigest,
    browser: SecretDigest,
    center: Option<String>,
    identity: ExternalIdentity,
    request_id: Uuid,
}

#[async_trait::async_trait]
impl IdentityUnitOfWork for CompleteExternal<'_> {
    type Output = ExternalCompletion;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        let at = now(self.service.clock.as_ref())?;
        let initial = transaction
            .external_flow_by_state_digest(&self.state_digest)
            .await?
            .ok_or(IdentityError::NotFound)?;
        initial.validate(&self.browser, at)?;
        if initial.provider_key != self.provider_key {
            return Err(IdentityError::Forbidden);
        }
        let mut keys = vec![IdentityLockKey::ExternalIdentity {
            provider_key: self.provider_key.as_str().into(),
            subject: self.identity.subject.clone(),
        }];
        if let Some(oidc_flow_id) = initial.oidc_flow_id {
            keys.push(IdentityLockKey::Flow(oidc_flow_id));
        }
        if let Some(user_id) = initial.user_id {
            keys.push(IdentityLockKey::User(user_id));
        }
        let center_digest = self
            .center
            .as_deref()
            .map(|secret| {
                validate_opaque(secret)?;
                Ok::<_, IdentityError>(SecretDigest::of(secret))
            })
            .transpose()?;
        let old_root = match &center_digest {
            Some(digest) => transaction.load_browser_session(digest).await?,
            None => None,
        };
        let mut plan: Option<IdentityRevocationPlan> = None;
        if let Some(root) = &old_root {
            keys.push(IdentityLockKey::Center(root.id));
            let value = transaction
                .revocation_plan(&IdentityRevocationScope::CurrentCenter {
                    sso_session_id: root.id,
                })
                .await?;
            keys.extend(value.locks.keys().to_vec());
            plan = Some(value);
        }
        let existing = if initial.purpose == ExternalPurpose::Login {
            transaction
                .external_identity_by_subject(self.provider_key, &self.identity.subject)
                .await?
        } else {
            None
        };
        if let Some(record) = &existing {
            keys.push(IdentityLockKey::User(record.user_id));
        }
        transaction
            .lock_identity_scope(&IdentityLockScope::new(keys)?)
            .await?;
        let flow = transaction
            .external_flow_by_state_digest(&self.state_digest)
            .await?
            .ok_or(IdentityError::NotFound)?;
        flow.validate(&self.browser, at)?;
        transaction
            .apply(IdentityWrite::ConsumeExternalFlow { id: flow.id })
            .await?;
        if let (Some(root), Some(plan)) = (&old_root, &plan) {
            let current = transaction
                .revocation_plan(&IdentityRevocationScope::CurrentCenter {
                    sso_session_id: root.id,
                })
                .await?;
            if current.locks.keys() != plan.locks.keys()
                || current.rp_session_ids != plan.rp_session_ids
            {
                return Err(IdentityError::Conflict);
            }
        }
        match flow.purpose {
            ExternalPurpose::Bind => self.bind(transaction, &flow, at).await,
            ExternalPurpose::Login => self.login(transaction, &flow, at, old_root, plan).await,
        }
    }
}

impl CompleteExternal<'_> {
    async fn bind(
        &self,
        transaction: &mut dyn IdentityTransaction,
        flow: &crate::domain::identity::external::ExternalFlowSnapshot,
        at: u64,
    ) -> Result<ExternalCompletion, IdentityError> {
        let expected_user = flow.user_id.ok_or(IdentityError::Forbidden)?;
        let secret = self
            .center
            .as_deref()
            .ok_or(IdentityError::Unauthenticated)?;
        let actor = authenticate_current_actor(
            transaction,
            &SecretDigest::of(secret),
            at,
            &IdentityLockScope::new(vec![IdentityLockKey::ExternalIdentity {
                provider_key: self.provider_key.as_str().into(),
                subject: self.identity.subject.clone(),
            }])?,
        )
        .await?;
        if actor.account().user_id != expected_user {
            return Err(IdentityError::Forbidden);
        }
        let existing = transaction
            .external_identity_by_subject(self.provider_key, &self.identity.subject)
            .await?;
        if let Some(record) = existing {
            if record.user_id != expected_user {
                append_audit(
                    transaction,
                    self.request_id,
                    Some(expected_user),
                    "external.bind",
                    "external_identity",
                    record.id.to_string(),
                    AuditResult::Denied,
                    Some(IdentityError::Conflict),
                    serde_json::json!({"provider": self.provider_key.as_str()}),
                )
                .await?;
                return Ok(ExternalCompletion::Rejected {
                    purpose: ExternalPurpose::Bind,
                    code: "conflict",
                });
            }
            append_audit(
                transaction,
                self.request_id,
                Some(expected_user),
                "external.bind",
                "external_identity",
                record.id.to_string(),
                AuditResult::Success,
                None,
                serde_json::json!({"provider": self.provider_key.as_str(), "idempotent": true}),
            )
            .await?;
            return Ok(ExternalCompletion::Bind {
                provider_key: self.provider_key,
            });
        }
        let bound = transaction
            .external_identities_for_user(expected_user)
            .await?;
        if bound
            .iter()
            .any(|record| record.provider_key == self.provider_key)
        {
            append_audit(
                transaction,
                self.request_id,
                Some(expected_user),
                "external.bind",
                "external_identity",
                Uuid::nil().to_string(),
                AuditResult::Denied,
                Some(IdentityError::Conflict),
                serde_json::json!({"provider": self.provider_key.as_str(), "existing": true}),
            )
            .await?;
            return Ok(ExternalCompletion::Rejected {
                purpose: ExternalPurpose::Bind,
                code: "conflict",
            });
        }
        let id = Uuid::new_v4();
        transaction
            .apply(IdentityWrite::CreateExternalIdentity(
                ExternalIdentityWrite {
                    id,
                    user_id: expected_user,
                    provider_key: self.provider_key,
                    subject: self.identity.subject.clone(),
                    union_id: self.identity.union_id.clone(),
                    display_name: self.identity.display_name.clone(),
                    avatar_url: self.identity.avatar_url.clone(),
                    created_at: at,
                    updated_at: at,
                    last_login_at: None,
                },
            ))
            .await?;
        append_audit(
            transaction,
            self.request_id,
            Some(expected_user),
            "external.bind",
            "external_identity",
            id.to_string(),
            AuditResult::Success,
            None,
            serde_json::json!({"provider": self.provider_key.as_str()}),
        )
        .await?;
        Ok(ExternalCompletion::Bind {
            provider_key: self.provider_key,
        })
    }

    async fn login(
        &self,
        transaction: &mut dyn IdentityTransaction,
        flow: &crate::domain::identity::external::ExternalFlowSnapshot,
        at: u64,
        old_root: Option<BrowserSessionSnapshot>,
        plan: Option<IdentityRevocationPlan>,
    ) -> Result<ExternalCompletion, IdentityError> {
        let existing = transaction
            .external_identity_by_subject(self.provider_key, &self.identity.subject)
            .await?;
        let (user_id, auth_epoch, identity_id) = match existing {
            Some(record) => {
                let account = transaction
                    .load_account(record.user_id)
                    .await?
                    .ok_or(IdentityError::Unauthenticated)?;
                ensure_enabled(&account)?;
                transaction
                    .apply(IdentityWrite::UpdateExternalIdentityLastLogin { id: record.id, at })
                    .await?;
                (record.user_id, account.auth_epoch, record.id)
            }
            None => match self.service.config.external_identity.registration {
                RegistrationPolicy::Disabled | RegistrationPolicy::InviteOnly => {
                    append_audit(
                        transaction,
                        self.request_id,
                        None,
                        "external.login",
                        "external_identity",
                        Uuid::nil().to_string(),
                        AuditResult::Denied,
                        Some(IdentityError::ExternalIdentityNotLinked),
                        serde_json::json!({"provider": self.provider_key.as_str()}),
                    )
                    .await?;
                    return Ok(ExternalCompletion::Rejected {
                        purpose: ExternalPurpose::Login,
                        code: "external_identity_not_linked",
                    });
                }
                RegistrationPolicy::Open => {
                    let username = format!("ext_{}", &Uuid::new_v4().simple().to_string()[..12]);
                    let nickname = self
                        .identity
                        .display_name
                        .clone()
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| "外部身份用户".to_owned());
                    let account = match transaction
                        .apply(IdentityWrite::CreateExternalAccount(
                            NewExternalAccountWrite {
                                username,
                                nickname,
                                person_kind:
                                    crate::domain::identity::account::PersonKind::Collaborator,
                            },
                        ))
                        .await?
                    {
                        crate::service::identity::repository::IdentityWriteResult::Account(
                            account,
                        ) => account,
                        _ => return Err(IdentityError::DependencyUnavailable),
                    };
                    let id = Uuid::new_v4();
                    transaction
                        .apply(IdentityWrite::CreateExternalIdentity(
                            ExternalIdentityWrite {
                                id,
                                user_id: account.user_id,
                                provider_key: self.provider_key,
                                subject: self.identity.subject.clone(),
                                union_id: self.identity.union_id.clone(),
                                display_name: self.identity.display_name.clone(),
                                avatar_url: self.identity.avatar_url.clone(),
                                created_at: at,
                                updated_at: at,
                                last_login_at: Some(at),
                            },
                        ))
                        .await?;
                    append_audit(
                        transaction,
                        self.request_id,
                        Some(account.user_id),
                        "external.account_create",
                        "user",
                        account.user_id.to_string(),
                        AuditResult::Success,
                        None,
                        serde_json::json!({"provider": self.provider_key.as_str()}),
                    )
                    .await?;
                    (account.user_id, account.auth_epoch, id)
                }
            },
        };
        if let (Some(root), Some(plan)) = (&old_root, &plan) {
            let scope = IdentityRevocationScope::CurrentCenter {
                sso_session_id: root.id,
            };
            transaction.apply(IdentityWrite::Revoke(scope)).await?;
            let event_id = Uuid::new_v4();
            append_audit(
                transaction,
                self.request_id,
                Some(user_id),
                "session.reauthenticate",
                "session",
                root.id.to_string(),
                AuditResult::Success,
                None,
                serde_json::json!({"affected_count": plan.rp_session_ids.len()}),
            )
            .await?;
            transaction
                .enqueue_logout(&LogoutEvent {
                    audit_event_id: event_id,
                    rp_session_ids: plan.rp_session_ids.clone(),
                })
                .await?;
        }
        let cookie_secret = IssuedSecret::from_entropy(rand::random());
        let expires_at = at
            .checked_add(self.service.config.identity.sso_session_ttl_seconds * 1000)
            .ok_or(IdentityError::CapacityExceeded)?;
        let root = BrowserSessionSnapshot {
            id: Uuid::new_v4(),
            principal: stable_principal(&self.service.config.issuer(), user_id)?,
            user_id,
            secret_digest: cookie_secret.digest(),
            auth_epoch,
            authenticated_at: at,
            expires_at,
            revoked_at: None,
        };
        transaction
            .apply(IdentityWrite::CreateCenter(root.clone()))
            .await?;
        if let Some(flow_id) = flow.oidc_flow_id {
            let current = transaction
                .load_authorization_flow(flow_id)
                .await?
                .ok_or(IdentityError::NotFound)?;
            if current.browser_binding_digest != self.browser {
                return Err(IdentityError::Forbidden);
            }
            if current.handle.completed_at.is_some()
                || current.bound_sso_session_id.is_some()
                || current.reauth_completed_at.is_some()
            {
                return Err(IdentityError::AlreadyConsumed);
            }
            transaction
                .apply(IdentityWrite::BindFlow {
                    flow_id,
                    sso_session_id: root.id,
                    reauthenticated: true,
                })
                .await?;
        }
        append_audit(
            transaction,
            self.request_id,
            Some(user_id),
            "external.login",
            "external_identity",
            identity_id.to_string(),
            AuditResult::Success,
            None,
            serde_json::json!({"provider": self.provider_key.as_str()}),
        )
        .await?;
        append_audit(
            transaction,
            self.request_id,
            Some(user_id),
            "session.login",
            "session",
            root.id.to_string(),
            AuditResult::Success,
            None,
            serde_json::json!({"user_id": user_id, "provider": self.provider_key.as_str()}),
        )
        .await?;
        Ok(ExternalCompletion::Login {
            cookie_secret,
            expires_at,
            oidc_flow_id: flow.oidc_flow_id,
        })
    }
}

struct ExternalFailure {
    provider_key: ExternalProviderKey,
    reason: IdentityError,
    request_id: Uuid,
}

#[async_trait::async_trait]
impl IdentityUnitOfWork for ExternalFailure {
    type Output = ();
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        append_audit(
            transaction,
            self.request_id,
            None,
            "external.login",
            "external_identity",
            Uuid::nil().to_string(),
            AuditResult::Failure,
            Some(self.reason),
            serde_json::json!({"provider": self.provider_key.as_str()}),
        )
        .await
    }
}

struct UnbindExternalIdentity<'a> {
    service: &'a ExternalIdentityService,
    user_secret: String,
    identity_id: Uuid,
    request_id: Uuid,
}

#[async_trait::async_trait]
impl IdentityUnitOfWork for UnbindExternalIdentity<'_> {
    type Output = ();
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        let at = now(self.service.clock.as_ref())?;
        let record = transaction
            .external_identity_by_id(self.identity_id)
            .await?
            .ok_or(IdentityError::NotFound)?;
        let actor = authenticate_current_actor(
            transaction,
            &SecretDigest::of(&self.user_secret),
            at,
            &IdentityLockScope::new(vec![
                IdentityLockKey::User(record.user_id),
                IdentityLockKey::ExternalIdentity {
                    provider_key: record.provider_key.as_str().into(),
                    subject: record.subject.clone(),
                },
            ])?,
        )
        .await?;
        if actor.account().user_id != record.user_id {
            return Err(IdentityError::Forbidden);
        }
        let remaining = transaction
            .external_login_credential_count(record.user_id)
            .await?;
        if remaining <= 1 {
            return Err(IdentityError::LastCredential);
        }
        transaction
            .apply(IdentityWrite::DeleteExternalIdentity {
                id: record.id,
                user_id: record.user_id,
            })
            .await?;
        append_audit(
            transaction,
            self.request_id,
            Some(record.user_id),
            "external.unbind",
            "external_identity",
            record.id.to_string(),
            AuditResult::Success,
            None,
            serde_json::json!({"provider": record.provider_key.as_str()}),
        )
        .await?;
        Ok(())
    }
}

fn view(record: ExternalIdentityRecord) -> ExternalIdentityView {
    ExternalIdentityView {
        id: record.id,
        provider_key: record.provider_key,
        provider_label: record.provider_key.label(),
        display_name: record.display_name,
        subject_hint: mask_subject(&record.subject),
        created_at: record.created_at / 1000,
        last_login_at: record.last_login_at.map(|value| value / 1000),
    }
}

fn mask_subject(subject: &str) -> String {
    if subject.len() <= 8 {
        return "***".to_owned();
    }
    format!("{}…{}", &subject[..4], &subject[subject.len() - 4..])
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn append_audit(
    transaction: &mut dyn IdentityTransaction,
    request_id: Uuid,
    actor_id: Option<i32>,
    action: &'static str,
    object_kind: &'static str,
    object_id: String,
    result: AuditResult,
    reason: Option<IdentityError>,
    change_summary: serde_json::Value,
) -> Result<(), IdentityError> {
    let id = Uuid::new_v4();
    transaction
        .append_audit(&AuditEvent {
            id,
            request_id,
            actor_id,
            action,
            object_kind,
            object_id,
            result,
            reason,
            change_summary,
            operation_id: None,
            commit_marker: Some(id),
        })
        .await
}
