//! Adapters translate one platform authorization into a stable external identity.
use crate::domain::identity::{
    error::IdentityError,
    external::{ExternalIdentity, ExternalProviderKey},
};
use auth::security::identity::SecretToken;
use std::sync::Arc;

#[derive(Clone)]
pub struct ExternalProviderConfig {
    pub key: ExternalProviderKey,
    pub client_id: String,
    pub secret: Arc<SecretToken>,
    /// Non-secret provider settings (endpoint overrides, scopes, launch options).
    pub config: serde_json::Value,
}

impl std::fmt::Debug for ExternalProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalProviderConfig")
            .field("key", &self.key)
            .field("client_id", &self.client_id)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

pub struct BeginContext<'a> {
    pub callback_uri: &'a str,
    pub state: &'a str,
    pub nonce: Option<&'a str>,
    pub code_challenge: Option<&'a str>,
    pub config: &'a ExternalProviderConfig,
}

pub struct NativeHint {
    pub kind: &'static str,
    pub title: &'static str,
    pub instruction: &'static str,
}

pub enum BeginOutcome {
    /// Browser providers return a third-party authorization URL.
    Redirect {
        authorize_url: String,
        /// Short-lived upstream secret (for example a PKCE verifier) to encrypt with the flow.
        secret_payload: Option<SecretToken>,
    },
    /// Native providers must be driven by the platform SDK, never by a browser.
    Native { hint: NativeHint },
}

pub struct CompleteContext<'a> {
    pub code: &'a str,
    pub callback_uri: &'a str,
    pub code_verifier: Option<&'a SecretToken>,
    pub nonce: Option<&'a str>,
    pub config: &'a ExternalProviderConfig,
    pub http: &'a super::http::ExternalHttpClient,
}

pub struct NativeCompleteContext<'a> {
    pub code: &'a str,
    pub config: &'a ExternalProviderConfig,
    pub http: &'a super::http::ExternalHttpClient,
}

#[derive(Debug, Clone, Copy)]
pub struct AdapterCapabilities {
    pub pkce: bool,
    pub nonce: bool,
    pub native: bool,
    pub union_id: bool,
}

#[async_trait::async_trait]
pub trait ExternalIdentityAdapter: Send + Sync {
    fn key(&self) -> ExternalProviderKey;

    fn capabilities(&self) -> AdapterCapabilities;

    async fn begin(&self, ctx: BeginContext<'_>) -> Result<BeginOutcome, IdentityError>;

    async fn complete(&self, ctx: CompleteContext<'_>) -> Result<ExternalIdentity, IdentityError>;

    async fn complete_native(
        &self,
        _ctx: NativeCompleteContext<'_>,
    ) -> Result<ExternalIdentity, IdentityError> {
        Err(IdentityError::InvalidInput)
    }
}
