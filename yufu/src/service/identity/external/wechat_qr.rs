//! WeChat Open Platform website-app QR login (`snsapi_login`).
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

const AUTHORIZE: &str = "https://open.weixin.qq.com/connect/qrconnect";
const TOKEN: &str = "https://api.weixin.qq.com/sns/oauth2/access_token";
const USERINFO: &str = "https://api.weixin.qq.com/sns/userinfo";
const SCOPE: &str = "snsapi_login";

pub struct WechatQrAdapter;

impl WechatQrAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WechatQrAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    openid: Option<String>,
    unionid: Option<String>,
    errcode: Option<i64>,
}

#[derive(Deserialize)]
struct UserInfo {
    nickname: Option<String>,
    headimgurl: Option<String>,
    unionid: Option<String>,
    errcode: Option<i64>,
}

#[async_trait::async_trait]
impl ExternalIdentityAdapter for WechatQrAdapter {
    fn key(&self) -> ExternalProviderKey {
        ExternalProviderKey::WechatQr
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
        let mut url = Url::parse(&authorize).map_err(|_| IdentityError::DependencyUnavailable)?;
        url.query_pairs_mut()
            .append_pair("appid", &ctx.config.client_id)
            .append_pair("redirect_uri", ctx.callback_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", &scope)
            .append_pair("state", ctx.state);
        url.set_fragment(Some("wechat_redirect"));
        Ok(BeginOutcome::Redirect {
            authorize_url: url.into(),
            secret_payload: None,
        })
    }

    async fn complete(&self, ctx: CompleteContext<'_>) -> Result<ExternalIdentity, IdentityError> {
        let token_endpoint = setting(ctx.config, "token_endpoint", TOKEN);
        let userinfo_endpoint = setting(ctx.config, "userinfo_endpoint", USERINFO);
        let lang = setting(ctx.config, "lang", "zh_CN");
        let mut token_url =
            Url::parse(&token_endpoint).map_err(|_| IdentityError::DependencyUnavailable)?;
        token_url
            .query_pairs_mut()
            .append_pair("appid", &ctx.config.client_id)
            .append_pair("secret", ctx.config.secret.expose_secret())
            .append_pair("code", ctx.code)
            .append_pair("grant_type", "authorization_code");
        let token: TokenResponse = ctx
            .http
            .get_json(token_url.as_str(), None)
            .await
            .map_err(|_| IdentityError::Unauthenticated)?;
        if token.errcode.unwrap_or(0) != 0 {
            return Err(IdentityError::Unauthenticated);
        }
        let access_token = token.access_token.ok_or(IdentityError::Unauthenticated)?;
        let openid = token.openid.ok_or(IdentityError::Unauthenticated)?;
        let mut userinfo_url =
            Url::parse(&userinfo_endpoint).map_err(|_| IdentityError::DependencyUnavailable)?;
        userinfo_url
            .query_pairs_mut()
            .append_pair("access_token", &access_token)
            .append_pair("openid", &openid)
            .append_pair("lang", &lang);
        let profile = ctx
            .http
            .get_json::<UserInfo>(userinfo_url.as_str(), None)
            .await
            .ok()
            .filter(|profile| profile.errcode.unwrap_or(0) == 0);
        let identity = ExternalIdentity {
            provider_key: ExternalProviderKey::WechatQr,
            subject: openid,
            union_id: token
                .unionid
                .or_else(|| profile.as_ref().and_then(|value| value.unionid.clone())),
            display_name: profile
                .as_ref()
                .and_then(|value| value.nickname.clone())
                .filter(|value| !value.trim().is_empty()),
            avatar_url: profile
                .as_ref()
                .and_then(|value| value.headimgurl.as_deref())
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
    use axum::{Json, Router, routing::get};
    use std::time::Duration;

    async fn fake_wechat(token_error: bool) -> u16 {
        let app = Router::new()
            .route(
                "/token",
                get(move || async move {
                    if token_error {
                        Json(serde_json::json!({"errcode":40029,"errmsg":"invalid code"}))
                    } else {
                        Json(serde_json::json!({
                            "access_token":"wechat-token",
                            "openid":"wx-openid-1",
                            "unionid":"wx-union-1"
                        }))
                    }
                }),
            )
            .route(
                "/userinfo",
                get(|| async {
                    Json(serde_json::json!({
                        "openid":"wx-openid-1",
                        "nickname":"微信用户",
                        "headimgurl":"http://thirdwx.qlogo.cn/avatar",
                        "unionid":"wx-union-1",
                        "errcode":0
                    }))
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
            key: ExternalProviderKey::WechatQr,
            client_id: "wx-appid".into(),
            secret: std::sync::Arc::new(
                auth::security::identity::SecretToken::new("wx-secret".into()).unwrap(),
            ),
            config: serde_json::json!({
                "authorize_endpoint": format!("http://127.0.0.1:{port}/authorize"),
                "token_endpoint": format!("http://127.0.0.1:{port}/token"),
                "userinfo_endpoint": format!("http://127.0.0.1:{port}/userinfo")
            }),
        }
    }

    #[tokio::test]
    async fn wechat_begin_uses_official_qrconnect_parameters() {
        let config = config(1);
        let outcome = WechatQrAdapter::new()
            .begin(BeginContext {
                callback_uri: "https://id.example/identity/external/wechat_qr/callback",
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
        assert_eq!(url.fragment(), Some("wechat_redirect"));
        let pairs = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(pairs.get("appid").unwrap(), "wx-appid");
        assert_eq!(pairs.get("scope").unwrap(), "snsapi_login");
        assert_eq!(pairs.get("state").unwrap(), "opaque-state");
        assert!(secret_payload.is_none());
    }

    #[tokio::test]
    async fn wechat_complete_reads_openid_unionid_and_normalizes_avatar() {
        let port = fake_wechat(false).await;
        let config = config(port);
        let http = ExternalHttpClient::new(Duration::from_secs(5), true).unwrap();
        let identity = WechatQrAdapter::new()
            .complete(CompleteContext {
                code: "code",
                callback_uri: "https://id.example/identity/external/wechat_qr/callback",
                code_verifier: None,
                nonce: None,
                config: &config,
                http: &http,
            })
            .await
            .unwrap();
        assert_eq!(identity.provider_key, ExternalProviderKey::WechatQr);
        assert_eq!(identity.subject, "wx-openid-1");
        assert_eq!(identity.union_id.as_deref(), Some("wx-union-1"));
        assert_eq!(identity.display_name.as_deref(), Some("微信用户"));
        assert_eq!(
            identity.avatar_url.as_deref(),
            Some("https://thirdwx.qlogo.cn/avatar")
        );
    }

    #[tokio::test]
    async fn wechat_token_error_is_not_a_login() {
        let port = fake_wechat(true).await;
        let config = config(port);
        let http = ExternalHttpClient::new(Duration::from_secs(5), true).unwrap();
        let result = WechatQrAdapter::new()
            .complete(CompleteContext {
                code: "code",
                callback_uri: "https://id.example/identity/external/wechat_qr/callback",
                code_verifier: None,
                nonce: None,
                config: &config,
                http: &http,
            })
            .await;
        assert_eq!(result.unwrap_err(), IdentityError::Unauthenticated);
    }
}
