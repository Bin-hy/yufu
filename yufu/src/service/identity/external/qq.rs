//! QQ Connect OAuth 2.0. Responses may be JSON or JSONP-wrapped.
use super::adapter::{
    AdapterCapabilities, BeginContext, BeginOutcome, CompleteContext, ExternalIdentityAdapter,
    ExternalProviderConfig,
};
use crate::domain::identity::{
    error::IdentityError,
    external::{ExternalIdentity, ExternalProviderKey},
};
use serde::Deserialize;
use url::Url;

const AUTHORIZE: &str = "https://graph.qq.com/oauth2.0/authorize";
const TOKEN: &str = "https://graph.qq.com/oauth2.0/token";
const ME: &str = "https://graph.qq.com/oauth2.0/me";
const USER: &str = "https://graph.qq.com/user/get_user_info";
const SCOPE: &str = "get_user_info";

pub struct QqAdapter;

impl QqAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for QqAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct MeResponse {
    openid: Option<String>,
    unionid: Option<String>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct UserResponse {
    ret: Option<i64>,
    nickname: Option<String>,
    figureurl_qq_2: Option<String>,
    figureurl_qq_1: Option<String>,
}

#[async_trait::async_trait]
impl ExternalIdentityAdapter for QqAdapter {
    fn key(&self) -> ExternalProviderKey {
        ExternalProviderKey::Qq
    }

    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            pkce: false,
            nonce: false,
            native: false,
            union_id: true,
        }
    }

    async fn begin(&self, ctx: BeginContext<'_>) -> Result<BeginOutcome, IdentityError> {
        let authorize = setting(ctx.config, "authorize_endpoint", AUTHORIZE);
        let scope = setting(ctx.config, "scope", SCOPE);
        let display = setting(ctx.config, "display", "pc");
        let mut url = Url::parse(&authorize).map_err(|_| IdentityError::DependencyUnavailable)?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &ctx.config.client_id)
            .append_pair("redirect_uri", ctx.callback_uri)
            .append_pair("state", ctx.state)
            .append_pair("scope", &scope)
            .append_pair("display", &display);
        Ok(BeginOutcome::Redirect {
            authorize_url: url.into(),
            secret_payload: None,
        })
    }

    async fn complete(&self, ctx: CompleteContext<'_>) -> Result<ExternalIdentity, IdentityError> {
        let token_endpoint = setting(ctx.config, "token_endpoint", TOKEN);
        let me_endpoint = setting(ctx.config, "me_endpoint", ME);
        let user_endpoint = setting(ctx.config, "user_endpoint", USER);
        let mut token_url =
            Url::parse(&token_endpoint).map_err(|_| IdentityError::DependencyUnavailable)?;
        token_url
            .query_pairs_mut()
            .append_pair("grant_type", "authorization_code")
            .append_pair("client_id", &ctx.config.client_id)
            .append_pair("client_secret", ctx.config.secret.expose_secret())
            .append_pair("code", ctx.code)
            .append_pair("redirect_uri", ctx.callback_uri)
            .append_pair("fmt", "json");
        let token: TokenResponse = parse_text(&ctx.http.get_text(token_url.as_str(), None).await?)?;
        if token.error.is_some() {
            return Err(IdentityError::Unauthenticated);
        }
        let access_token = token.access_token.ok_or(IdentityError::Unauthenticated)?;
        let mut me_url =
            Url::parse(&me_endpoint).map_err(|_| IdentityError::DependencyUnavailable)?;
        me_url
            .query_pairs_mut()
            .append_pair("access_token", &access_token)
            .append_pair("unionid", "1")
            .append_pair("fmt", "json");
        let me: MeResponse = parse_text(&ctx.http.get_text(me_url.as_str(), None).await?)?;
        if me.error.is_some() {
            return Err(IdentityError::Unauthenticated);
        }
        let openid = me.openid.ok_or(IdentityError::Unauthenticated)?;
        let mut user_url =
            Url::parse(&user_endpoint).map_err(|_| IdentityError::DependencyUnavailable)?;
        user_url
            .query_pairs_mut()
            .append_pair("access_token", &access_token)
            .append_pair("oauth_consumer_key", &ctx.config.client_id)
            .append_pair("openid", &openid);
        let user: UserResponse = parse_text(&ctx.http.get_text(user_url.as_str(), None).await?)?;
        if user.ret.unwrap_or(0) != 0 {
            return Err(IdentityError::Unauthenticated);
        }
        let identity = ExternalIdentity {
            provider_key: ExternalProviderKey::Qq,
            subject: openid,
            union_id: me.unionid,
            display_name: user.nickname.filter(|value| !value.trim().is_empty()),
            avatar_url: user
                .figureurl_qq_2
                .or(user.figureurl_qq_1)
                .as_deref()
                .and_then(normalize_avatar),
            email: None,
            email_verified: false,
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

fn parse_text<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, IdentityError> {
    serde_json::from_str(strip_jsonp(text)).map_err(|_| IdentityError::DependencyUnavailable)
}

fn strip_jsonp(text: &str) -> &str {
    let trimmed = text.trim();
    let Some(open) = trimmed.find('(') else {
        return trimmed;
    };
    let Some(close) = trimmed.rfind(')') else {
        return trimmed;
    };
    if close <= open {
        return trimmed;
    }
    let inner = trimmed[open + 1..close].trim();
    let prefix = trimmed[..open].trim();
    if prefix.starts_with("callback") || prefix.starts_with("jsonp") {
        inner
    } else {
        trimmed
    }
}

fn normalize_avatar(value: &str) -> Option<String> {
    let value = value.trim();
    if value.starts_with("https://") {
        Some(value.to_owned())
    } else {
        value
            .strip_prefix("http://")
            .map(|rest| format!("https://{rest}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::identity::external::http::ExternalHttpClient;
    use axum::{Router, response::IntoResponse, routing::get};
    use std::time::Duration;

    async fn fake_qq(jsonp: bool) -> u16 {
        let body = |value: &'static str| {
            if jsonp {
                format!("callback( {value} );")
            } else {
                value.to_owned()
            }
        };
        let token = body(r#"{"access_token":"qq-token"}"#);
        let me = body(r#"{"client_id":"qq-appid","openid":"qq-openid","unionid":"qq-union"}"#);
        let user = r#"{"ret":0,"nickname":"QQ用户","figureurl_qq_2":"http://qlogo.example/a.png"}"#;
        let app = Router::new()
            .route(
                "/token",
                get(move || {
                    let token = token.clone();
                    async move { token.into_response() }
                }),
            )
            .route(
                "/me",
                get(move || {
                    let me = me.clone();
                    async move { me.into_response() }
                }),
            )
            .route(
                "/user",
                get(move || {
                    let user = user.to_owned();
                    async move { user.into_response() }
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
            key: ExternalProviderKey::Qq,
            client_id: "qq-appid".into(),
            secret: std::sync::Arc::new(
                auth::security::identity::SecretToken::new("qq-appkey".into()).unwrap(),
            ),
            config: serde_json::json!({
                "authorize_endpoint": format!("http://127.0.0.1:{port}/authorize"),
                "token_endpoint": format!("http://127.0.0.1:{port}/token"),
                "me_endpoint": format!("http://127.0.0.1:{port}/me"),
                "user_endpoint": format!("http://127.0.0.1:{port}/user")
            }),
        }
    }

    #[tokio::test]
    async fn qq_begin_uses_state_and_fixed_callback() {
        let config = config(1);
        let outcome = QqAdapter::new()
            .begin(BeginContext {
                callback_uri: "https://id.example/identity/external/qq/callback",
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
        assert_eq!(pairs.get("client_id").unwrap(), "qq-appid");
        assert_eq!(pairs.get("state").unwrap(), "opaque-state");
        assert_eq!(pairs.get("scope").unwrap(), "get_user_info");
        assert_eq!(pairs.get("display").unwrap(), "pc");
        assert!(secret_payload.is_none());
    }

    #[tokio::test]
    async fn qq_complete_accepts_jsonp_and_json() {
        for jsonp in [true, false] {
            let port = fake_qq(jsonp).await;
            let config = config(port);
            let http = ExternalHttpClient::new(Duration::from_secs(5), true).unwrap();
            let identity = QqAdapter::new()
                .complete(CompleteContext {
                    code: "code",
                    callback_uri: "https://id.example/identity/external/qq/callback",
                    code_verifier: None,
                    nonce: None,
                    config: &config,
                    http: &http,
                })
                .await
                .unwrap();
            assert_eq!(identity.provider_key, ExternalProviderKey::Qq);
            assert_eq!(identity.subject, "qq-openid");
            assert_eq!(identity.union_id.as_deref(), Some("qq-union"));
            assert_eq!(identity.display_name.as_deref(), Some("QQ用户"));
            assert_eq!(
                identity.avatar_url.as_deref(),
                Some("https://qlogo.example/a.png")
            );
        }
    }

    #[test]
    fn jsonp_extraction_only_unwraps_callback_wrappers() {
        assert_eq!(strip_jsonp("callback( {\"a\":1} );"), "{\"a\":1}");
        assert_eq!(strip_jsonp("{\"a\":1}"), "{\"a\":1}");
        assert_eq!(strip_jsonp("jsonp_1({\"a\":1})"), "{\"a\":1}");
        assert_eq!(strip_jsonp("not a wrapper"), "not a wrapper");
    }
}
