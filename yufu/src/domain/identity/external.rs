//! External identity vocabulary. Provider subjects never become Yufu subjects.
use super::account::{PersonKind, SecretDigest};
use super::error::IdentityError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ExternalProviderKey {
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "qq")]
    Qq,
    #[serde(rename = "wechat_mini")]
    WechatMini,
    #[serde(rename = "wechat_web")]
    WechatWeb,
    #[serde(rename = "wechat_qr")]
    WechatQr,
}

impl ExternalProviderKey {
    pub const ALL: &'static [Self] = &[
        Self::GitHub,
        Self::Qq,
        Self::WechatMini,
        Self::WechatWeb,
        Self::WechatQr,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::Qq => "qq",
            Self::WechatMini => "wechat_mini",
            Self::WechatWeb => "wechat_web",
            Self::WechatQr => "wechat_qr",
        }
    }

    pub const fn family(self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::Qq => "qq",
            Self::WechatMini | Self::WechatWeb | Self::WechatQr => "wechat",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::GitHub => "GitHub",
            Self::Qq => "QQ",
            Self::WechatMini => "微信小程序",
            Self::WechatWeb => "微信网页",
            Self::WechatQr => "微信扫码",
        }
    }

    /// Native adapters are driven by a platform SDK; a browser cannot start them.
    pub const fn is_native(self) -> bool {
        matches!(self, Self::WechatMini)
    }
}

impl fmt::Display for ExternalProviderKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ExternalProviderKey {
    type Err = IdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "github" => Ok(Self::GitHub),
            "qq" => Ok(Self::Qq),
            "wechat_mini" => Ok(Self::WechatMini),
            "wechat_web" => Ok(Self::WechatWeb),
            "wechat_qr" => Ok(Self::WechatQr),
            _ => Err(IdentityError::InvalidInput),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalPurpose {
    Login,
    Bind,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationPolicy {
    #[default]
    Disabled,
    InviteOnly,
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalIdentity {
    pub provider_key: ExternalProviderKey,
    /// Stable platform identifier: GitHub numeric id, QQ openid, WeChat openid.
    pub subject: String,
    pub union_id: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub email: Option<String>,
    pub email_verified: bool,
}

impl ExternalIdentity {
    pub fn validate(&self) -> Result<(), IdentityError> {
        validate_subject(&self.subject)?;
        if let Some(union_id) = &self.union_id {
            validate_subject(union_id)?;
        }
        if self
            .display_name
            .as_deref()
            .is_some_and(|value| value.chars().count() > 200 || value.chars().any(char::is_control))
        {
            return Err(IdentityError::InvalidInput);
        }
        if self.avatar_url.as_deref().is_some_and(|value| {
            value.len() > 2048
                || !value.starts_with("https://")
                || value.chars().any(char::is_control)
        }) {
            return Err(IdentityError::InvalidInput);
        }
        if self.email.as_deref().is_some_and(|value| {
            value.len() > 254 || value.chars().any(char::is_control) || !value.contains('@')
        }) {
            return Err(IdentityError::InvalidInput);
        }
        Ok(())
    }
}

fn validate_subject(value: &str) -> Result<(), IdentityError> {
    if value.is_empty()
        || value.len() > 255
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return Err(IdentityError::InvalidInput);
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ExternalFlowSnapshot {
    pub id: Uuid,
    pub provider_key: ExternalProviderKey,
    pub purpose: ExternalPurpose,
    pub browser_binding_digest: SecretDigest,
    pub user_id: Option<i32>,
    pub oidc_flow_id: Option<Uuid>,
    pub nonce: Option<String>,
    pub secret_ciphertext: Option<String>,
    pub expires_at: u64,
    pub consumed_at: Option<u64>,
}

impl ExternalFlowSnapshot {
    pub fn validate(&self, browser: &SecretDigest, now_millis: u64) -> Result<(), IdentityError> {
        if &self.browser_binding_digest != browser {
            return Err(IdentityError::Forbidden);
        }
        if self.consumed_at.is_some() {
            return Err(IdentityError::AlreadyConsumed);
        }
        if now_millis >= self.expires_at {
            return Err(IdentityError::Expired);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ExternalFlowWrite {
    pub id: Uuid,
    pub provider_key: ExternalProviderKey,
    pub purpose: ExternalPurpose,
    pub state_digest: SecretDigest,
    pub browser_binding_digest: SecretDigest,
    pub user_id: Option<i32>,
    pub oidc_flow_id: Option<Uuid>,
    pub nonce: Option<String>,
    pub secret_ciphertext: Option<String>,
    pub expires_at: u64,
}

impl ExternalFlowWrite {
    pub fn validate(&self, now_millis: u64) -> Result<(), IdentityError> {
        if self.id.is_nil()
            || self.expires_at <= now_millis
            || self.expires_at - now_millis > 3_600_000
            || (self.purpose == ExternalPurpose::Bind && self.user_id.is_none())
        {
            return Err(IdentityError::InvalidInput);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ExternalIdentityRecord {
    pub id: Uuid,
    pub user_id: i32,
    pub provider_key: ExternalProviderKey,
    pub subject: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: u64,
    pub last_login_at: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ExternalIdentityWrite {
    pub id: Uuid,
    pub user_id: i32,
    pub provider_key: ExternalProviderKey,
    pub subject: String,
    pub union_id: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub last_login_at: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct NewExternalAccountWrite {
    pub username: String,
    pub nickname: String,
    pub person_kind: PersonKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExternalProviderAdminView {
    pub provider_key: String,
    pub family: String,
    pub enabled: bool,
    pub client_id: String,
    pub secret_configured: bool,
    pub config_complete: bool,
    pub revision: u64,
    pub updated_at: u64,
    pub updated_by: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateExternalProvider {
    pub expected_revision: u64,
    pub enabled: bool,
    pub client_id: String,
    pub secret: Option<String>,
}

impl UpdateExternalProvider {
    pub fn validate(&self, provider_key: ExternalProviderKey) -> Result<(), IdentityError> {
        if provider_key.is_native() {
            return Err(IdentityError::InvalidInput);
        }
        if self.client_id.len() > 256 || self.client_id.chars().any(char::is_control) {
            return Err(IdentityError::InvalidInput);
        }
        if self.enabled && self.client_id.trim().is_empty() {
            return Err(IdentityError::InvalidInput);
        }
        if let Some(secret) = &self.secret
            && (secret.is_empty() || secret.len() > 512 || secret.chars().any(char::is_control))
        {
            return Err(IdentityError::InvalidInput);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RotateExternalProviderSecret {
    pub expected_revision: u64,
    pub secret: String,
}

impl RotateExternalProviderSecret {
    pub fn validate(&self) -> Result<(), IdentityError> {
        if self.secret.is_empty()
            || self.secret.len() > 512
            || self.secret.chars().any(char::is_control)
        {
            return Err(IdentityError::InvalidInput);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ExternalProviderStats {
    pub from: u64,
    pub to: u64,
    pub providers: Vec<ProviderStat>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderStat {
    pub provider_key: String,
    pub success: u64,
    pub denied: u64,
    pub failure: u64,
    pub reasons: Vec<(String, u64)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> ExternalIdentity {
        ExternalIdentity {
            provider_key: ExternalProviderKey::GitHub,
            subject: "123456".into(),
            union_id: None,
            display_name: Some("octocat".into()),
            avatar_url: Some("https://avatars.example/u/1".into()),
            email: Some("octocat@example.test".into()),
            email_verified: true,
        }
    }

    #[test]
    fn provider_keys_round_trip_and_stay_distinct() {
        for key in ExternalProviderKey::ALL {
            assert_eq!(key.as_str().parse::<ExternalProviderKey>(), Ok(*key));
            assert_eq!(
                serde_json::to_string(key).unwrap(),
                format!("\"{}\"", key.as_str())
            );
        }
        assert_eq!(ExternalProviderKey::WechatMini.family(), "wechat");
        assert!(ExternalProviderKey::WechatMini.is_native());
        assert!(!ExternalProviderKey::Qq.is_native());
        assert!("unknown".parse::<ExternalProviderKey>().is_err());
    }

    #[test]
    fn external_identity_rejects_unsafe_subjects_and_urls() {
        assert!(identity().validate().is_ok());
        for subject in ["", "has space", "a\nb", "中文", &"a".repeat(256)] {
            let mut value = identity();
            value.subject = subject.into();
            assert!(value.validate().is_err(), "{subject}");
        }
        let mut value = identity();
        value.avatar_url = Some("http://insecure.example/a.png".into());
        assert!(value.validate().is_err());
        let mut value = identity();
        value.email = Some("not-an-email".into());
        assert!(value.validate().is_err());
        let mut value = identity();
        value.display_name = Some("line\nbreak".into());
        assert!(value.validate().is_err());
    }

    #[test]
    fn registration_policy_defaults_to_disabled() {
        assert_eq!(RegistrationPolicy::default(), RegistrationPolicy::Disabled);
        assert_eq!(
            serde_json::to_string(&RegistrationPolicy::InviteOnly).unwrap(),
            "\"invite_only\""
        );
    }
}
