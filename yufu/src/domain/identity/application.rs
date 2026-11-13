use super::account::{IssuedSecret, ManagementMode};
use super::error::IdentityError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use url::{Host, Url};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AssignmentTarget {
    User(i32),
    Group(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppEnvironmentKind {
    Development,
    Production,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    ServerWeb,
    BrowserSpa,
}
impl ClientKind {
    pub fn permits_introspection_reader(self) -> bool {
        matches!(self, Self::ServerWeb | Self::BrowserSpa)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentPolicy {
    UserConsent,
    EnterprisePreconsent,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogoutCapability {
    BackchannelAndOnline,
    Online,
    LocalOnly,
}

/// Preserves the exact registered string, including its query and trailing slash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ExactRedirectUri(String);

/// An application entry point, independently versioned from OIDC callbacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct LaunchUrl(String);

impl LaunchUrl {
    pub fn parse(
        value: impl Into<String>,
        environment: AppEnvironmentKind,
    ) -> Result<Self, IdentityError> {
        let value = value.into();
        if !value.contains("://") {
            return Err(IdentityError::InvalidInput);
        }
        parse_http_uri(&value, environment)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn parse_http_uri(value: &str, environment: AppEnvironmentKind) -> Result<Url, IdentityError> {
    if value.is_empty()
        || value.len() > 2048
        || value.chars().any(|c| c.is_whitespace() || c.is_control())
        || value.contains('*')
        || value.contains('\\')
        || value.contains('#')
    {
        return Err(IdentityError::InvalidInput);
    }
    let decoded = urlencoding::decode(value).map_err(|_| IdentityError::InvalidInput)?;
    if decoded.contains('*') || decoded.chars().any(char::is_control) {
        return Err(IdentityError::InvalidInput);
    }
    let uri = Url::parse(value).map_err(|_| IdentityError::InvalidInput)?;
    if uri.host().is_none()
        || !uri.username().is_empty()
        || uri.password().is_some()
        || uri.fragment().is_some()
    {
        return Err(IdentityError::InvalidInput);
    }
    let loopback = match uri.host() {
        Some(Host::Domain("localhost")) => true,
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        _ => false,
    };
    if uri.scheme() != "https"
        && !(environment == AppEnvironmentKind::Development && uri.scheme() == "http" && loopback)
    {
        return Err(IdentityError::InvalidInput);
    }
    Ok(uri)
}

impl ExactRedirectUri {
    pub fn parse(
        value: impl Into<String>,
        environment: AppEnvironmentKind,
    ) -> Result<Self, IdentityError> {
        let value = value.into();
        parse_http_uri(&value, environment)?;
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn matches(&self, candidate: &str) -> bool {
        self.0 == candidate
    }
    fn origin(&self) -> Result<String, IdentityError> {
        Ok(Url::parse(&self.0)
            .map_err(|_| IdentityError::InvalidInput)?
            .origin()
            .ascii_serialization())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ExactOrigin(String);
impl ExactOrigin {
    pub fn parse(
        value: impl Into<String>,
        environment: AppEnvironmentKind,
    ) -> Result<Self, IdentityError> {
        let value = value.into();
        let uri = parse_http_uri(&value, environment)?;
        if uri.query().is_some() || uri.origin().ascii_serialization() != value {
            return Err(IdentityError::InvalidInput);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn matches(&self, candidate: &str) -> bool {
        self.0 == candidate
    }
}

/// Syntax approval only. Delivery must also enforce administrator network policy
/// on DNS answers and the actual connected address before sending a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ApprovedDeliveryUri(String);
impl ApprovedDeliveryUri {
    pub fn parse(value: impl Into<String>) -> Result<Self, IdentityError> {
        let value = value.into();
        parse_http_uri(&value, AppEnvironmentKind::Production)?;
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Audience(String);
impl Audience {
    pub fn parse(value: impl Into<String>) -> Result<Self, IdentityError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 512
            || value.contains('*')
            || value.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(IdentityError::InvalidInput);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct IdentityScopeSet(BTreeSet<String>);
impl IdentityScopeSet {
    pub fn parse(value: &str) -> Result<Self, IdentityError> {
        if value.len() > 128
            || value
                .chars()
                .any(|c| c.is_control() || (c.is_whitespace() && c != ' '))
        {
            return Err(IdentityError::InvalidInput);
        }
        let scopes: BTreeSet<String> = value
            .split(' ')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        if !scopes.contains("openid")
            || scopes
                .iter()
                .any(|s| !matches!(s.as_str(), "openid" | "profile" | "email"))
        {
            return Err(IdentityError::InvalidInput);
        }
        Ok(Self(scopes))
    }
    pub fn contains(&self, scope: &str) -> bool {
        self.0.contains(scope)
    }
    pub fn is_subset_of(&self, approved: &Self) -> bool {
        self.0.is_subset(&approved.0)
    }
    pub fn as_protocol_string(&self) -> String {
        self.0
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnvironmentConfig {
    pub launch_url: Option<LaunchUrl>,
    pub launch_description: Option<String>,
    pub client_kind: ClientKind,
    pub redirect_uris: Vec<ExactRedirectUri>,
    pub post_logout_redirect_uris: Vec<ExactRedirectUri>,
    pub browser_origins: Vec<ExactOrigin>,
    pub access_audience: Audience,
    pub identity_scopes: IdentityScopeSet,
    pub consent_policy: ConsentPolicy,
    pub logout_capability: LogoutCapability,
    pub backchannel_logout_uri: Option<ApprovedDeliveryUri>,
}

/// Wire data is validated for its environment before it becomes a config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentConfigInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_description: Option<String>,
    pub client_kind: ClientKind,
    pub redirect_uris: Vec<String>,
    pub post_logout_redirect_uris: Vec<String>,
    pub browser_origins: Vec<String>,
    pub access_audience: String,
    pub identity_scopes: String,
    pub consent_policy: ConsentPolicy,
    pub logout_capability: LogoutCapability,
    pub backchannel_logout_uri: Option<String>,
}

impl EnvironmentConfigInput {
    pub fn validate(
        self,
        environment: AppEnvironmentKind,
        activation: bool,
    ) -> Result<EnvironmentConfig, IdentityError> {
        if (activation && self.redirect_uris.is_empty())
            || self.redirect_uris.len() > 16
            || self.post_logout_redirect_uris.len() > 16
            || self.browser_origins.len() > 16
        {
            return Err(IdentityError::InvalidInput);
        }
        fn unique(values: &[String]) -> bool {
            values.iter().collect::<BTreeSet<_>>().len() == values.len()
        }
        if !unique(&self.redirect_uris)
            || !unique(&self.post_logout_redirect_uris)
            || !unique(&self.browser_origins)
        {
            return Err(IdentityError::InvalidInput);
        }
        let redirect_uris = self
            .redirect_uris
            .into_iter()
            .map(|s| ExactRedirectUri::parse(s, environment))
            .collect::<Result<Vec<_>, _>>()?;
        let post_logout_redirect_uris = self
            .post_logout_redirect_uris
            .into_iter()
            .map(|s| ExactRedirectUri::parse(s, environment))
            .collect::<Result<Vec<_>, _>>()?;
        let browser_origins = self
            .browser_origins
            .into_iter()
            .map(|s| ExactOrigin::parse(s, environment))
            .collect::<Result<Vec<_>, _>>()?;
        if self.client_kind == ClientKind::BrowserSpa {
            let origins = redirect_uris
                .iter()
                .map(ExactRedirectUri::origin)
                .collect::<Result<BTreeSet<_>, _>>()?;
            let configured: BTreeSet<_> = browser_origins.iter().map(|o| o.0.clone()).collect();
            if (activation && configured.is_empty()) || origins != configured {
                return Err(IdentityError::InvalidInput);
            }
        } else if !browser_origins.is_empty() {
            return Err(IdentityError::InvalidInput);
        }
        let backchannel_logout_uri = self
            .backchannel_logout_uri
            .map(ApprovedDeliveryUri::parse)
            .transpose()?;
        if (self.logout_capability == LogoutCapability::BackchannelAndOnline)
            != backchannel_logout_uri.is_some()
        {
            return Err(IdentityError::InvalidInput);
        }
        let launch_url = self
            .launch_url
            .filter(|value| !value.is_empty())
            .map(|value| LaunchUrl::parse(value, environment))
            .transpose()?;
        if self
            .launch_description
            .as_ref()
            .is_some_and(|value| value.chars().any(char::is_control))
        {
            return Err(IdentityError::InvalidInput);
        }
        let launch_description = self
            .launch_description
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if launch_description.as_ref().is_some_and(|value| {
            value.chars().count() > 1000 || value.chars().any(char::is_control)
        }) {
            return Err(IdentityError::InvalidInput);
        }
        Ok(EnvironmentConfig {
            launch_url,
            launch_description,
            client_kind: self.client_kind,
            redirect_uris,
            post_logout_redirect_uris,
            browser_origins,
            access_audience: Audience::parse(self.access_audience)?,
            identity_scopes: IdentityScopeSet::parse(&self.identity_scopes)?,
            consent_policy: self.consent_policy,
            logout_capability: self.logout_capability,
            backchannel_logout_uri,
        })
    }
}

impl EnvironmentConfig {
    pub fn to_input(&self) -> EnvironmentConfigInput {
        EnvironmentConfigInput {
            launch_url: self.launch_url.as_ref().map(|url| url.as_str().to_owned()),
            launch_description: self.launch_description.clone(),
            client_kind: self.client_kind,
            redirect_uris: self
                .redirect_uris
                .iter()
                .map(|uri| uri.as_str().to_owned())
                .collect(),
            post_logout_redirect_uris: self
                .post_logout_redirect_uris
                .iter()
                .map(|uri| uri.as_str().to_owned())
                .collect(),
            browser_origins: self
                .browser_origins
                .iter()
                .map(|origin| origin.as_str().to_owned())
                .collect(),
            access_audience: self.access_audience.as_str().to_owned(),
            identity_scopes: self.identity_scopes.as_protocol_string(),
            consent_policy: self.consent_policy,
            logout_capability: self.logout_capability,
            backchannel_logout_uri: self
                .backchannel_logout_uri
                .as_ref()
                .map(|uri| uri.as_str().to_owned()),
        }
    }
    pub fn to_payload(&self) -> Result<serde_json::Value, IdentityError> {
        serde_json::to_value(self.to_input()).map_err(|_| IdentityError::InvalidInput)
    }
    pub fn from_payload(
        payload: serde_json::Value,
        environment: AppEnvironmentKind,
        activation: bool,
    ) -> Result<Self, IdentityError> {
        serde_json::from_value::<EnvironmentConfigInput>(payload)
            .map_err(|_| IdentityError::DependencyUnavailable)?
            .validate(environment, activation)
    }
    pub fn accepts_redirect(&self, value: &str) -> bool {
        self.redirect_uris.iter().any(|uri| uri.matches(value))
    }
    pub fn accepts_logout_redirect(&self, value: &str) -> bool {
        self.post_logout_redirect_uris
            .iter()
            .any(|uri| uri.matches(value))
    }
    pub fn accepts_origin(&self, value: &str) -> bool {
        self.client_kind == ClientKind::BrowserSpa
            && self
                .browser_origins
                .iter()
                .any(|origin| origin.matches(value))
    }
    pub fn permits_client_secret(&self) -> bool {
        self.client_kind == ClientKind::ServerWeb
    }
    pub fn validate_authorization(
        &self,
        response_type: &str,
        pkce_method: &str,
        scopes: &IdentityScopeSet,
    ) -> Result<(), IdentityError> {
        if response_type != "code"
            || pkce_method != "S256"
            || !scopes.is_subset_of(&self.identity_scopes)
        {
            return Err(IdentityError::InvalidInput);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalSnapshot {
    pub environment_id: Uuid,
    pub config_version_id: Uuid,
    pub config_hash: String,
    pub environment_revision: u64,
    pub owner_user_id: i32,
    pub owner_revision: u64,
    pub access_revision: u64,
    pub security_revision: u64,
    pub logout_limitation_accepted: bool,
}

impl ApprovalSnapshot {
    pub fn digest(&self) -> Result<String, IdentityError> {
        let bytes = serde_json::to_vec(self).map_err(|_| IdentityError::InvalidInput)?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }
}

pub fn validate_review(
    requested_by: i32,
    reviewed_by: i32,
    saved: &ApprovalSnapshot,
    current: &ApprovalSnapshot,
    logout: LogoutCapability,
) -> Result<(), IdentityError> {
    if requested_by <= 0 || reviewed_by <= 0 {
        return Err(IdentityError::InvalidInput);
    }
    if requested_by == reviewed_by {
        return Err(IdentityError::SelfApproval);
    }
    if saved != current {
        return Err(IdentityError::ApprovalObsolete);
    }
    if logout == LogoutCapability::LocalOnly && !saved.logout_limitation_accepted {
        return Err(IdentityError::InvalidInput);
    }
    Ok(())
}

pub fn requires_candidate_client(active: ClientKind, proposed: ClientKind) -> bool {
    active != proposed
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentSummary {
    pub environment_id: Uuid,
    pub kind: AppEnvironmentKind,
    pub client_id: String,
    pub active_version_id: Option<Uuid>,
}

pub struct RegistrationReceipt {
    pub operation_id: Uuid,
    pub application_id: i32,
    pub management_mode: ManagementMode,
    pub environments: Vec<EnvironmentSummary>,
    pub newly_issued_secrets: Vec<(Uuid, IssuedSecret)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn product_launch_payload() {
        let legacy = input()
            .validate(AppEnvironmentKind::Production, true)
            .unwrap()
            .to_payload()
            .unwrap();
        assert!(legacy.get("launch_url").is_none());
        let loaded =
            EnvironmentConfig::from_payload(legacy.clone(), AppEnvironmentKind::Production, true)
                .unwrap();
        assert!(loaded.launch_url.is_none());
        assert!(loaded.launch_description.is_none());
        assert_eq!(loaded.to_payload().unwrap(), legacy);
        let mut changed = input();
        changed.launch_url = Some("https://app.example/home?tab=one".into());
        changed.launch_description = Some("  应用工作台  ".into());
        let changed = changed
            .validate(AppEnvironmentKind::Production, true)
            .unwrap();
        assert_eq!(changed.launch_description.as_deref(), Some("应用工作台"));
        assert_ne!(changed.to_payload().unwrap(), legacy);
        assert_eq!(
            EnvironmentConfig::from_payload(
                changed.to_payload().unwrap(),
                AppEnvironmentKind::Production,
                true
            )
            .unwrap(),
            changed
        );
        for description in ["a".repeat(1001), "hello\n".into(), "\tworkbench".into()] {
            let mut bad = input();
            bad.launch_description = Some(description);
            assert!(bad.validate(AppEnvironmentKind::Production, true).is_err());
        }
        let mut empty = input();
        empty.launch_url = Some(String::new());
        empty.launch_description = Some("  ".into());
        assert_eq!(
            empty
                .validate(AppEnvironmentKind::Production, true)
                .unwrap()
                .to_payload()
                .unwrap(),
            legacy
        );
    }
    #[test]
    fn product_launch_url() {
        for value in ["https://app.example/", "https://app.example/home?q=one"] {
            assert_eq!(
                LaunchUrl::parse(value, AppEnvironmentKind::Production)
                    .unwrap()
                    .as_str(),
                value
            );
        }
        for value in [
            "http://localhost:3000/",
            "http://127.0.0.2/",
            "http://[::1]/",
        ] {
            assert!(LaunchUrl::parse(value, AppEnvironmentKind::Development).is_ok());
            assert!(LaunchUrl::parse(value, AppEnvironmentKind::Production).is_err());
        }
        for value in [
            "",
            "/home",
            "//app.example/home",
            "javascript:alert(1)",
            "https:app.example/home",
            "http://app.example/",
            "https://user:pass@app.example/",
            "https://app.example/#",
            "https://app.example/\n",
            "https://app.example/%0d",
            "https://app.example/\\home",
            " https://app.example/",
        ] {
            assert!(
                LaunchUrl::parse(value, AppEnvironmentKind::Development).is_err(),
                "{value:?}"
            );
        }
        assert!(
            LaunchUrl::parse(
                format!("https://app.example/{}", "a".repeat(2048)),
                AppEnvironmentKind::Production
            )
            .is_err()
        );
    }

    pub(crate) fn input() -> EnvironmentConfigInput {
        EnvironmentConfigInput {
            client_kind: ClientKind::ServerWeb,
            redirect_uris: vec!["https://app.example/callback?channel=a".into()],
            post_logout_redirect_uris: vec!["https://app.example/logged-out".into()],
            browser_origins: vec![],
            access_audience: "urn:app:production".into(),
            identity_scopes: "openid profile email".into(),
            consent_policy: ConsentPolicy::UserConsent,
            logout_capability: LogoutCapability::Online,
            backchannel_logout_uri: None,
            launch_url: None,
            launch_description: None,
        }
    }

    #[test]
    fn test_exact_redirect_rejects_wildcards_credentials_fragments_and_insecure_production() {
        for uri in [
            "",
            "https://app.example/*",
            "https://app.example/%2A",
            "https://user:pass@app.example/cb",
            "https://app.example/cb#",
            "http://app.example/cb",
            "http://127.0.0.1/cb",
            " https://app.example/cb",
            "javascript:alert(1)",
            "https://app.example/\\evil",
        ] {
            assert!(
                ExactRedirectUri::parse(uri, AppEnvironmentKind::Production).is_err(),
                "{uri}"
            );
        }
        let uri =
            ExactRedirectUri::parse("https://app.example/cb?x=1", AppEnvironmentKind::Production)
                .unwrap();
        assert!(uri.matches("https://app.example/cb?x=1"));
        assert!(!uri.matches("https://app.example/cb?x=2"));
        assert!(!uri.matches("https://APP.example/cb?x=1"));
    }

    #[test]
    fn test_development_http_is_explicit_loopback_only() {
        for uri in [
            "http://localhost:4000/cb",
            "http://127.0.0.2/cb",
            "http://[::1]:4000/cb",
            "https://app.example/cb",
        ] {
            assert!(
                ExactRedirectUri::parse(uri, AppEnvironmentKind::Development).is_ok(),
                "{uri}"
            );
        }
        for uri in [
            "http://localhost.evil/cb",
            "http://0.0.0.0/cb",
            "http://192.168.1.1/cb",
        ] {
            assert!(
                ExactRedirectUri::parse(uri, AppEnvironmentKind::Development).is_err(),
                "{uri}"
            );
        }
    }

    #[test]
    fn test_origin_requires_exact_origin_without_path_or_query() {
        let origin =
            ExactOrigin::parse("https://app.example", AppEnvironmentKind::Production).unwrap();
        assert!(origin.matches("https://app.example"));
        assert!(!origin.matches("https://app.example/"));
        for value in [
            "https://app.example/",
            "https://app.example/path",
            "https://app.example?x=y",
            "null",
            "https://APP.example",
            "https://app.example:443",
        ] {
            assert!(
                ExactOrigin::parse(value, AppEnvironmentKind::Production).is_err(),
                "{value}"
            );
        }
    }

    #[test]
    fn test_identity_scope_subset_and_pkce_never_grant_business_access() {
        for scopes in [
            "",
            "profile",
            "openid offline_access",
            "openid mcp:access",
            "openid admin",
            "openid\temail",
        ] {
            assert!(IdentityScopeSet::parse(scopes).is_err(), "{scopes}");
        }
        let config = input()
            .validate(AppEnvironmentKind::Production, true)
            .unwrap();
        let requested = IdentityScopeSet::parse("openid email").unwrap();
        assert!(requested.contains("email"));
        assert!(requested.is_subset_of(&config.identity_scopes));
        assert_eq!(requested.as_protocol_string(), "email openid");
        assert!(
            config
                .validate_authorization("code", "S256", &requested)
                .is_ok()
        );
        assert!(
            config
                .validate_authorization("token", "S256", &requested)
                .is_err()
        );
        assert!(
            config
                .validate_authorization("code", "plain", &requested)
                .is_err()
        );
        let mut limited = input();
        limited.identity_scopes = "openid".into();
        assert!(
            limited
                .validate(AppEnvironmentKind::Production, true)
                .unwrap()
                .validate_authorization("code", "S256", &requested)
                .is_err()
        );
    }

    #[test]
    fn test_spa_has_no_secret_and_only_approved_callback_origins() {
        let mut spa = input();
        spa.client_kind = ClientKind::BrowserSpa;
        assert!(
            spa.clone()
                .validate(AppEnvironmentKind::Production, true)
                .is_err()
        );
        spa.browser_origins = vec!["https://app.example".into()];
        let config = spa
            .clone()
            .validate(AppEnvironmentKind::Production, true)
            .unwrap();
        assert!(!config.permits_client_secret());
        assert!(config.accepts_origin("https://app.example"));
        assert!(!config.accepts_origin("https://evil.example"));
        spa.browser_origins.push("https://evil.example".into());
        assert!(spa.validate(AppEnvironmentKind::Production, true).is_err());
        let config = input()
            .validate(AppEnvironmentKind::Production, true)
            .unwrap();
        assert!(config.permits_client_secret());
        assert!(!config.accepts_origin("https://app.example"));
        assert!(config.accepts_redirect("https://app.example/callback?channel=a"));
        assert!(!config.accepts_logout_redirect("https://app.example/callback?channel=a"));
        assert!(config.accepts_logout_redirect("https://app.example/logged-out"));
    }

    #[test]
    fn test_production_draft_is_distinct_from_activation_and_logout_delivery_is_explicit() {
        let mut draft = input();
        draft.redirect_uris.clear();
        assert!(
            draft
                .clone()
                .validate(AppEnvironmentKind::Production, false)
                .is_ok()
        );
        assert!(
            draft
                .validate(AppEnvironmentKind::Production, true)
                .is_err()
        );
        let mut delivery = input();
        delivery.logout_capability = LogoutCapability::BackchannelAndOnline;
        assert!(
            delivery
                .clone()
                .validate(AppEnvironmentKind::Production, true)
                .is_err()
        );
        delivery.backchannel_logout_uri = Some("https://app.example/backchannel".into());
        let config = delivery
            .clone()
            .validate(AppEnvironmentKind::Production, true)
            .unwrap();
        assert_eq!(
            config.backchannel_logout_uri.unwrap().as_str(),
            "https://app.example/backchannel"
        );
        delivery.backchannel_logout_uri = Some("http://127.0.0.1/backchannel".into());
        assert!(
            delivery
                .validate(AppEnvironmentKind::Development, true)
                .is_err()
        );
    }

    #[test]
    fn test_review_rejects_self_approval_and_changed_snapshot() {
        let snapshot = ApprovalSnapshot {
            environment_id: Uuid::new_v4(),
            config_version_id: Uuid::new_v4(),
            config_hash: "hash".into(),
            environment_revision: 3,
            owner_user_id: 5,
            owner_revision: 2,
            access_revision: 8,
            security_revision: 4,
            logout_limitation_accepted: false,
        };
        assert_eq!(
            validate_review(5, 5, &snapshot, &snapshot, LogoutCapability::Online),
            Err(IdentityError::SelfApproval)
        );
        assert!(validate_review(5, 7, &snapshot, &snapshot, LogoutCapability::Online).is_ok());
        for field in 0..7 {
            let mut changed = snapshot.clone();
            match field {
                0 => changed.owner_user_id += 1,
                1 => changed.owner_revision += 1,
                2 => changed.access_revision += 1,
                3 => changed.security_revision += 1,
                4 => changed.environment_revision += 1,
                5 => changed.config_version_id = Uuid::new_v4(),
                _ => changed.logout_limitation_accepted = true,
            }
            assert_eq!(
                validate_review(5, 7, &snapshot, &changed, LogoutCapability::Online),
                Err(IdentityError::ApprovalObsolete)
            );
            assert_ne!(snapshot.digest().unwrap(), changed.digest().unwrap());
        }
        assert_eq!(
            validate_review(5, 7, &snapshot, &snapshot, LogoutCapability::LocalOnly),
            Err(IdentityError::InvalidInput)
        );
        assert!(requires_candidate_client(
            ClientKind::ServerWeb,
            ClientKind::BrowserSpa
        ));
        assert!(!requires_candidate_client(
            ClientKind::ServerWeb,
            ClientKind::ServerWeb
        ));
    }

    #[test]
    fn test_config_limits_duplicates_and_audience() {
        for audience in ["", "urn:*", "urn:app secret"] {
            assert!(Audience::parse(audience).is_err());
        }
        assert_eq!(Audience::parse("urn:app").unwrap().as_str(), "urn:app");
        let mut duplicate = input();
        duplicate
            .redirect_uris
            .push(duplicate.redirect_uris[0].clone());
        assert!(
            duplicate
                .validate(AppEnvironmentKind::Production, true)
                .is_err()
        );
        let mut too_many = input();
        too_many.redirect_uris = (0..17)
            .map(|n| format!("https://app.example/cb/{n}"))
            .collect();
        assert!(
            too_many
                .validate(AppEnvironmentKind::Production, true)
                .is_err()
        );
    }

    #[test]
    fn test_persisted_config_is_revalidated_for_its_environment() {
        let config = input()
            .validate(AppEnvironmentKind::Production, true)
            .unwrap();
        assert_eq!(
            EnvironmentConfig::from_payload(
                config.to_payload().unwrap(),
                AppEnvironmentKind::Production,
                true
            )
            .unwrap(),
            config
        );
        let mut dev = input();
        dev.redirect_uris = vec!["http://localhost:4000/cb".into()];
        let dev = dev.validate(AppEnvironmentKind::Development, true).unwrap();
        assert!(
            EnvironmentConfig::from_payload(
                dev.to_payload().unwrap(),
                AppEnvironmentKind::Production,
                true
            )
            .is_err()
        );
        assert!(
            EnvironmentConfig::from_payload(
                serde_json::json!({}),
                AppEnvironmentKind::Development,
                true
            )
            .is_err()
        );
    }
}
