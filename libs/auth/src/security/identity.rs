use std::fmt;

use serde::{Deserialize, Serialize};

use super::error::SecurityError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalKind {
    User,
    Machine,
    Device,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenUse {
    UserSession,
    McpAccess,
    SecurityService,
    PermissionSync,
    DeviceAccess,
    GatewayOperation,
    StreamConnect,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IdentityBinding {
    None,
    Gateway {
        gateway_id: String,
    },
    Device {
        gateway_id: String,
        device_id: String,
    },
}

/// Namespaced identity, not proof of authentication. Constructed fields are validated;
/// signature and current credential status are checked separately by the verifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "PrincipalWire")]
pub struct PrincipalRef {
    issuer: String,
    kind: PrincipalKind,
    subject: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrincipalWire {
    issuer: String,
    kind: PrincipalKind,
    subject: String,
}

impl PrincipalRef {
    pub fn new(
        issuer: impl Into<String>,
        kind: PrincipalKind,
        subject: impl Into<String>,
    ) -> Result<Self, SecurityError> {
        let issuer = issuer.into();
        let subject = subject.into();
        let url = reqwest::Url::parse(&issuer).map_err(|_| SecurityError::InvalidCredential)?;
        if !matches!(url.scheme(), "https" | "http")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || issuer.chars().any(|c| c.is_whitespace() || c.is_control())
            || subject.trim().is_empty()
            || subject.chars().any(char::is_control)
        {
            return Err(SecurityError::InvalidCredential);
        }
        // Preserve the exact issuer string: URL normalization must not merge namespaces.
        Ok(Self {
            issuer,
            kind,
            subject,
        })
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }
    pub fn kind(&self) -> PrincipalKind {
        self.kind
    }
    pub fn subject(&self) -> &str {
        &self.subject
    }
}

impl TryFrom<PrincipalWire> for PrincipalRef {
    type Error = SecurityError;

    fn try_from(raw: PrincipalWire) -> Result<Self, Self::Error> {
        Self::new(raw.issuer, raw.kind, raw.subject)
    }
}

/// Deliberately has no Serialize, Deserialize or Deref implementation.
/// Explicit exposure is reserved for verifier and authenticated transport adapters.
pub struct SecretToken(String);

impl SecretToken {
    pub fn new(value: String) -> Result<Self, SecurityError> {
        if value.is_empty()
            || value.chars().any(char::is_whitespace)
            || value.chars().any(char::is_control)
        {
            return Err(SecurityError::InvalidCredential);
        }
        Ok(Self(value))
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretToken([REDACTED])")
    }
}

impl fmt::Display for SecretToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_t03_principal_namespaces_are_distinct() {
        let user = PrincipalRef::new("https://idp.test", PrincipalKind::User, "42").unwrap();
        let machine = PrincipalRef::new("https://idp.test", PrincipalKind::Machine, "42").unwrap();
        let other = PrincipalRef::new("https://other.test", PrincipalKind::User, "42").unwrap();
        assert_ne!(user, machine);
        assert_ne!(user, other);
        assert_eq!(user.subject(), "42");
    }

    #[test]
    fn security_t03_deserialization_validates_identity() {
        for issuer in [
            "",
            "not an issuer",
            "https://user:password@idp.test",
            "https://idp.test/#fragment",
        ] {
            assert!(PrincipalRef::new(issuer, PrincipalKind::User, "42").is_err());
        }
        for subject in ["", " ", "a\nb"] {
            let raw =
                serde_json::json!({"issuer":"https://idp.test","kind":"user","subject":subject});
            assert!(serde_json::from_value::<PrincipalRef>(raw).is_err());
        }
        let principal = PrincipalRef::new(
            "https://idp.test/tenant/",
            PrincipalKind::Device,
            "robot-42",
        )
        .unwrap();
        let roundtrip: PrincipalRef =
            serde_json::from_value(serde_json::to_value(&principal).unwrap()).unwrap();
        assert_eq!(principal, roundtrip);
        assert_eq!(roundtrip.issuer(), "https://idp.test/tenant/");
    }

    #[test]
    fn security_t03_secret_formatting_is_redacted() {
        let secret = SecretToken::new("sensitive-test-bearer".to_owned()).unwrap();
        assert!(!format!("{secret:?} {secret}").contains("sensitive-test-bearer"));
        assert_eq!(secret.expose_secret(), "sensitive-test-bearer");
        assert!(SecretToken::new(String::new()).is_err());
    }
}
