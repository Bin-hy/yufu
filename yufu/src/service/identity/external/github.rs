//! GitHub OAuth 2.0 (not OIDC): the stable identity is the numeric user id.
use super::adapter::{
    AdapterCapabilities, BeginContext, BeginOutcome, CompleteContext, ExternalIdentityAdapter,
    ExternalProviderConfig,
};
use crate::domain::identity::{
    error::IdentityError,
    external::{ExternalIdentity, ExternalProviderKey},
};
use auth::security::identity::SecretToken;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{RngCore, rngs::OsRng};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

const AUTHORIZE: &str = "https://github.com/login/oauth/authorize";
const TOKEN: &str = "https://github.com/login/oauth/access_token";
const USER: &str = "https://api.github.com/user";
const EMAILS: &str = "https://api.github.com/user/emails";
const SCOPE: &str = "read:user user:email";

pub struct GitHubAdapter;

impl GitHubAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitHubAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct GitHubUser {
    id: u64,
    login: String,
    name: Option<String>,
    avatar_url: Option<String>,
    email: Option<String>,
}

#[derive(Deserialize)]
struct GitHubEmail {
    email: String,
    primary: bool,
    verified: bool,
}

#[async_trait::async_trait]
impl ExternalIdentityAdapter for GitHubAdapter {
    fn key(&self) -> ExternalProviderKey {
        ExternalProviderKey::GitHub
    }

    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            pkce: true,
            nonce: false,
            native: false,
            union_id: false,
        }
    }

    async fn begin(&self, ctx: BeginContext<'_>) -> Result<BeginOutcome, IdentityError> {
        let authorize = setting(ctx.config, "authorize_endpoint", AUTHORIZE);
        let scope = setting(ctx.config, "scope", SCOPE);
        let use_pkce = ctx
            .config
            .config
            .get("pkce")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let mut verifier = [0u8; 32];
        OsRng.fill_bytes(&mut verifier);
        let verifier = URL_SAFE_NO_PAD.encode(verifier);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let mut url = Url::parse(&authorize).map_err(|_| IdentityError::DependencyUnavailable)?;
        url.query_pairs_mut()
            .append_pair("client_id", &ctx.config.client_id)
            .append_pair("redirect_uri", ctx.callback_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", &scope)
            .append_pair("state", ctx.state);
        if ctx
            .config
            .config
            .get("prompt")
            .and_then(serde_json::Value::as_str)
            == Some("select_account")
        {
            url.query_pairs_mut()
                .append_pair("prompt", "select_account");
        }
        if use_pkce {
            url.query_pairs_mut()
                .append_pair("code_challenge", &challenge)
                .append_pair("code_challenge_method", "S256");
        }
        Ok(BeginOutcome::Redirect {
            authorize_url: url.into(),
            secret_payload: use_pkce
                .then(|| {
                    SecretToken::new(verifier).map_err(|_| IdentityError::DependencyUnavailable)
                })
                .transpose()?,
        })
    }

    async fn complete(&self, ctx: CompleteContext<'_>) -> Result<ExternalIdentity, IdentityError> {
        let token_endpoint = setting(ctx.config, "token_endpoint", TOKEN);
        let user_endpoint = setting(ctx.config, "user_endpoint", USER);
        let emails_endpoint = setting(ctx.config, "emails_endpoint", EMAILS);
        let mut form = vec![
            ("client_id", ctx.config.client_id.as_str()),
            ("client_secret", ctx.config.secret.expose_secret()),
            ("code", ctx.code),
            ("redirect_uri", ctx.callback_uri),
        ];
        if let Some(verifier) = ctx.code_verifier {
            form.push(("code_verifier", verifier.expose_secret()));
        }
        let token: TokenResponse = ctx
            .http
            .post_form(&token_endpoint, &form, true)
            .await
            .map_err(|_| {
                tracing::warn!("github token request failed");
                IdentityError::Unauthenticated
            })?;
        if let Some(error) = &token.error {
            tracing::warn!(
                error = %error,
                description = ?token.error_description,
                "github token exchange rejected"
            );
            return Err(IdentityError::Unauthenticated);
        }
        let access_token = token
            .access_token
            .ok_or(IdentityError::Unauthenticated)
            .and_then(|value| {
                SecretToken::new(value).map_err(|_| IdentityError::Unauthenticated)
            })?;
        let user: GitHubUser = ctx
            .http
            .get_json(&user_endpoint, Some(&access_token))
            .await
            .map_err(|_| {
                tracing::warn!("github user request failed");
                IdentityError::Unauthenticated
            })?;
        let mut email = user.email;
        let mut email_verified = false;
        if let Ok(emails) = ctx
            .http
            .get_json::<Vec<GitHubEmail>>(&emails_endpoint, Some(&access_token))
            .await
            && let Some(primary) = emails
                .into_iter()
                .find(|email| email.primary && email.verified)
        {
            email_verified = true;
            email = Some(primary.email);
        }
        let identity = ExternalIdentity {
            provider_key: ExternalProviderKey::GitHub,
            subject: user.id.to_string(),
            union_id: None,
            display_name: user
                .name
                .filter(|value| !value.trim().is_empty())
                .or(Some(user.login)),
            avatar_url: user.avatar_url,
            email,
            email_verified,
        };
        identity.validate()?;
        Ok(identity)
    }
}

fn setting(config: &ExternalProviderConfig, key: &str, default: &str) -> String {
    config
        .config
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::identity::external::http::ExternalHttpClient;
    use axum::{
        Json, Router,
        routing::{get, post},
    };
    use std::time::Duration;

    async fn fake_github(token_error: bool) -> u16 {
        let app = Router::new()
            .route(
                "/token",
                post(move || async move {
                    if token_error {
                        Json(serde_json::json!({"error":"bad_verification_code"}))
                    } else {
                        Json(serde_json::json!({
                            "access_token":"gho_test",
                            "token_type":"bearer",
                            "scope":"read:user user:email"
                        }))
                    }
                }),
            )
            .route(
                "/user",
                get(|| async {
                    Json(serde_json::json!({
                        "id":42,
                        "login":"octocat",
                        "name":"Octo Cat",
                        "avatar_url":"https://avatars.example/u/42",
                        "email":null
                    }))
                }),
            )
            .route(
                "/emails",
                get(|| async {
                    Json(serde_json::json!([
                        {"email":"old@example.test","primary":false,"verified":true},
                        {"email":"primary@example.test","primary":true,"verified":true}
                    ]))
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        port
    }

    fn config(port: u16) -> ExternalProviderConfig {
        ExternalProviderConfig {
            key: ExternalProviderKey::GitHub,
            client_id: "client-id".into(),
            secret: std::sync::Arc::new(SecretToken::new("client-secret".into()).unwrap()),
            config: serde_json::json!({
                "authorize_endpoint": format!("http://127.0.0.1:{port}/authorize"),
                "token_endpoint": format!("http://127.0.0.1:{port}/token"),
                "user_endpoint": format!("http://127.0.0.1:{port}/user"),
                "emails_endpoint": format!("http://127.0.0.1:{port}/emails"),
                "prompt": "select_account"
            }),
        }
    }

    #[tokio::test]
    async fn github_begin_uses_state_pkce_and_fixed_callback() {
        let config = config(1);
        let outcome = GitHubAdapter::new()
            .begin(BeginContext {
                callback_uri: "https://id.example/identity/external/github/callback",
                state: "opaque-state",
                nonce: None,
                code_challenge: None,
                config: &config,
            })
            .await
            .unwrap();
        let BeginOutcome::Redirect {
            authorize_url,
            secret_payload,
        } = outcome
        else {
            panic!("expected redirect");
        };
        let url = Url::parse(&authorize_url).unwrap();
        let pairs = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(pairs.get("client_id").unwrap(), "client-id");
        assert_eq!(pairs.get("state").unwrap(), "opaque-state");
        assert_eq!(pairs.get("code_challenge_method").unwrap(), "S256");
        assert_eq!(pairs.get("prompt").unwrap(), "select_account");
        assert_eq!(
            pairs.get("redirect_uri").unwrap(),
            "https://id.example/identity/external/github/callback"
        );
        assert!(secret_payload.is_some());
    }

    #[tokio::test]
    async fn github_complete_reads_stable_id_and_verified_primary_email() {
        let port = fake_github(false).await;
        let config = config(port);
        let http = ExternalHttpClient::new(Duration::from_secs(5), true).unwrap();
        let verifier = SecretToken::new("verifier".into()).unwrap();
        let identity = GitHubAdapter::new()
            .complete(CompleteContext {
                code: "code",
                callback_uri: "http://127.0.0.1:4000/identity/external/github/callback",
                code_verifier: Some(&verifier),
                nonce: None,
                config: &config,
                http: &http,
            })
            .await
            .unwrap();
        assert_eq!(identity.provider_key, ExternalProviderKey::GitHub);
        assert_eq!(identity.subject, "42");
        assert_eq!(identity.display_name.as_deref(), Some("Octo Cat"));
        assert_eq!(identity.email.as_deref(), Some("primary@example.test"));
        assert!(identity.email_verified);
    }

    #[tokio::test]
    async fn github_token_error_is_not_a_login() {
        let port = fake_github(true).await;
        let config = config(port);
        let http = ExternalHttpClient::new(Duration::from_secs(5), true).unwrap();
        let result = GitHubAdapter::new()
            .complete(CompleteContext {
                code: "code",
                callback_uri: "http://127.0.0.1:4000/identity/external/github/callback",
                code_verifier: None,
                nonce: None,
                config: &config,
                http: &http,
            })
            .await;
        assert_eq!(result.unwrap_err(), IdentityError::Unauthenticated);
    }
}
