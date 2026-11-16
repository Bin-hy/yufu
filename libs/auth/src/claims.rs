use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};

/// Represents the payload (claims) contained within a JWT token.
///
/// The `Claims` struct stores essential information about the token,
/// such as the user ID and expiration time.
///
/// # Fields
///
/// * `id` - The unique identifier of the user or subject associated with the token.
/// * `exp` - The UNIX timestamp (in seconds) when the token will expire.
///
/// # Example
///
/// ```
/// use auth::claims::Claims;
///
/// let claims = Claims {
///     id: "user123".to_string(),
///     exp: 1_700_003_600,
/// };
///
/// println!("{}", claims);
/// ```
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Claims<IDTYPE>
where
    IDTYPE: Clone + Send + Sync + 'static + Serialize,
{
    pub id: IDTYPE,
    pub exp: u64, // expiration
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RefreshClaims<IDTYPE>
where
    IDTYPE: Clone + Send + Sync + 'static + Serialize,
{
    pub jti: String, // refresh token id, for SSO
    pub sub: IDTYPE, // user_id
    pub exp: u64,    // expiration
}

impl<IDTYPE> Display for Claims<IDTYPE>
where
    IDTYPE: Clone + Send + Sync + 'static + Serialize + for<'de> Deserialize<'de> + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "id: {:?}, expire: {}", self.id, self.exp)
    }
}

/// Standard JWT access token claims (RFC 7519).
///
/// Used for RS256-signed access tokens that cross service boundaries.
/// Verified by holders of the IdP's public key (via JWKS) without any
/// shared secret. `sub` is a string per OIDC; parse to your local ID type
/// (e.g. `claims.sub.parse::<i32>()`) when needed.
///
/// `roles` and `permissions` are embedded at token issuance so downstream
/// services can make coarse-grained local decisions without calling the PDP.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AccessTokenClaims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub iat: u64,
    pub exp: u64,
    pub jti: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub scope: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<String>,
}

impl Display for AccessTokenClaims {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "iss={} sub={} aud={} jti={} exp={}",
            self.iss, self.sub, self.aud, self.jti, self.exp
        )
    }
}

/// OIDC ID Token claims (RFC 7519 + OpenID Connect Core 1.0)
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct IdTokenClaims {
    pub iss: String,         // issuer
    pub sub: String,         // OIDC subject; convert local user IDs at the boundary
    pub aud: Option<String>, // audience (client_id)
    pub iat: u64,            // issued at
    pub exp: u64,            // expiration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>, // replay protection
    pub at_hash: Option<String>, // access token hash
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_time: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
}

#[derive(Clone, Default)]
pub struct IdentityProfile {
    pub name: Option<String>,
    pub preferred_username: Option<String>,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
}

impl IdentityProfile {
    /// Synthesizes `<preferred_username>@<domain>` for accounts without a stored
    /// address.
    ///
    /// Several relying parties (for example Outline) reject an OIDC login whose
    /// profile carries no `email` claim, so an account created without one would
    /// be unable to sign in anywhere. Historical deployments answered with
    /// `{username}@{email_domain}`; this keeps that contract and covers every
    /// profile consumer at once, because the fallback lives on the value that is
    /// loaded once per flow.
    ///
    /// The synthesized address is never authoritative: it is reported as
    /// unverified, and it leaves an explicitly stored address untouched. An empty
    /// or blank domain disables the fallback.
    pub fn with_email_domain(mut self, domain: &str) -> Self {
        let domain = domain.trim();
        if self.email.is_some() || domain.is_empty() {
            return self;
        }
        let username = self
            .preferred_username
            .as_deref()
            .map(str::trim)
            .filter(|username| !username.is_empty());
        if let Some(username) = username {
            self.email = Some(format!("{username}@{domain}"));
            self.email_verified = Some(false);
        }
        self
    }
}

/// An ID Token's signed session reference, never an authentication credential.
pub struct LogoutReference {
    pub issuer: String,
    pub subject: String,
    pub client_id: String,
    pub protocol_sid: uuid::Uuid,
    pub auth_time: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogoutTokenClaims {
    pub iss: String,
    pub aud: String,
    pub iat: u64,
    pub exp: u64,
    pub jti: String,
    pub sub: String,
    pub sid: String,
    pub events: std::collections::BTreeMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::IdentityProfile;

    fn profile(username: Option<&str>, email: Option<&str>) -> IdentityProfile {
        IdentityProfile {
            name: None,
            preferred_username: username.map(str::to_owned),
            email: email.map(str::to_owned),
            email_verified: email.map(|_| true),
        }
    }

    #[test]
    fn identity_profile_synthesizes_missing_address() {
        let filled = profile(Some("cargo"), None).with_email_domain("binhy.email");
        assert_eq!(filled.email.as_deref(), Some("cargo@binhy.email"));
        // A synthesized address is never reported as verified.
        assert_eq!(filled.email_verified, Some(false));
    }

    #[test]
    fn identity_profile_keeps_stored_address_and_verification() {
        let kept =
            profile(Some("cargo"), Some("real@example.test")).with_email_domain("binhy.email");
        assert_eq!(kept.email.as_deref(), Some("real@example.test"));
        assert_eq!(kept.email_verified, Some(true));
    }

    #[test]
    fn identity_profile_skips_fallback_without_a_domain_or_username() {
        assert!(
            profile(Some("cargo"), None)
                .with_email_domain("")
                .email
                .is_none()
        );
        assert!(
            profile(Some("cargo"), None)
                .with_email_domain("   ")
                .email
                .is_none()
        );
        assert!(
            profile(None, None)
                .with_email_domain("binhy.email")
                .email
                .is_none()
        );
        assert!(
            profile(Some("  "), None)
                .with_email_domain("binhy.email")
                .email
                .is_none()
        );
    }

    #[test]
    fn identity_profile_trims_domain() {
        let trimmed = profile(Some("cargo"), None).with_email_domain(" binhy.email ");
        assert_eq!(trimmed.email.as_deref(), Some("cargo@binhy.email"));
    }
}
