use serde::{Deserialize, Serialize};
use std::fmt::Display;

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
pub struct Claims {
    pub id: String,
    pub exp: u64, // expiration
}

impl Display for Claims {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "id: {} expire:{}", self.id, self.exp)
    }
}
