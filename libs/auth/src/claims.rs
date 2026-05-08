use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};

/// Represents the payload (claims) contained within a JWT token.
///
/// The `Claims` struct stores essential information about the token,
/// such as the user ID, the issued-at timestamp, and the expiration time.
///
/// # Fields
///
/// * `id` - The unique identifier of the user or subject associated with the token.
/// * `iat` - The UNIX timestamp (in seconds) when the token was issued.
/// * `exp` - The UNIX timestamp (in seconds) when the token will expire.
///
/// # Example
///
/// ```
/// use auth::Claims;
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
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct IdTokenClaims<IDTYPE>
where
    IDTYPE: Clone + Send + Sync + 'static + Serialize,
{
    pub iss: String,             // issuer
    pub sub: IDTYPE,             // subject (user_id)
    pub aud: Option<String>,     // audience (client_id)
    pub iat: u64,                // issued at
    pub exp: u64,                // expiration
    pub nonce: Option<String>,   // replay protection
    pub at_hash: Option<String>, // access token hash
}
