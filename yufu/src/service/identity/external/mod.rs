//! External identity federation: provider adapters, encrypted configuration and guardrails.
pub mod adapter;
pub mod admin;
pub mod bootstrap;
pub mod github;
pub mod http;
pub mod qq;
pub mod rate_limit;
pub mod registry;
pub mod secret;
pub mod service;
pub mod wechat_qr;

pub use adapter::{
    AdapterCapabilities, BeginContext, BeginOutcome, CompleteContext, ExternalIdentityAdapter,
    ExternalProviderConfig, NativeCompleteContext, NativeHint,
};
pub use admin::{ExternalProviderAdminService, StatsWindow};
pub use github::GitHubAdapter;
pub use http::ExternalHttpClient;
pub use qq::QqAdapter;
pub use rate_limit::ExternalRateLimiter;
pub use registry::{ExternalIdentityRegistry, PublicProvider, StoredProvider};
pub use secret::SecretCipher;
pub use service::{
    ExternalCallbackContext, ExternalCompletion, ExternalIdentityService, ExternalIdentityView,
};
pub use wechat_qr::WechatQrAdapter;
