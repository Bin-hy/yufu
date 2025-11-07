use crate::claims::Claims;

use chrono::{Duration, Utc};
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{
    DecodingKey, EncodingKey, Header, Validation, decode, encode, errors::Result as JwtResult,
};
use serde::{Deserialize, Serialize};

/// Token manager for centralized creation and validation of JWT tokens.
#[derive(Clone)]
pub struct TokenManager {
    pub secret: String,
    pub expire_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TokenError {
    Expired,
    InvalidSignature,
    InvalidToken,
    Other(String),
}

impl TokenManager {
    /// Creates a new [`TokenManager`].
    ///
    /// # Arguments
    ///
    /// * `secret` - Secret key used to sign the JWT token.
    /// * `expire_seconds` - Token expiration time in seconds.
    ///
    /// # Example
    ///
    /// ```
    /// use auth::TokenManager;
    ///
    /// let manager = TokenManager::new("mysecret", 3600);
    /// ```
    pub fn new(secret: impl Into<String>, expire_seconds: i64) -> Self {
        Self {
            secret: secret.into(),
            expire_seconds,
        }
    }

    /// Creates a new JWT token for a given user ID.
    ///
    /// # Arguments
    ///
    /// * `user_id` - The ID of the user for whom the token will be generated.
    ///
    /// # Returns
    ///
    /// Returns a signed JWT token string if successful.
    ///
    /// # Example
    ///
    /// ```
    /// use auth::TokenManager;
    ///
    /// let manager = TokenManager::new("mysecret", 3600);
    /// let token = manager.generate_token("user123").unwrap();
    /// println!("Generated token: {}", token);
    /// ```
    pub fn generate_token(&self, user_id: &str) -> JwtResult<String> {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.expire_seconds);
        let claims = Claims {
            id: user_id.to_string(),
            exp: exp.timestamp() as u64,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
    }

    /// Validates a JWT token and returns the decoded [`Claims`] if valid.
    ///
    /// # Arguments
    ///
    /// * `token` - The JWT token string to verify.
    ///
    /// # Returns
    ///
    /// Returns [`Claims`] on success, or a [`TokenError`] on failure.
    ///
    /// # Errors
    ///
    /// * [`TokenError::Expired`] - The token has expired.
    /// * [`TokenError::InvalidSignature`] - The signature is invalid.
    /// * [`TokenError::InvalidToken`] - The token format or content is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use auth::{TokenManager, TokenError};
    ///
    /// let manager = TokenManager::new("mysecret", 3600);
    /// let token = manager.generate_token("user1").unwrap();
    ///
    /// match manager.verify_token(&token) {
    ///     Ok(claims) => println!("Token valid, id: {}", claims.id),
    ///     Err(TokenError::Expired) => println!("Token expired, please log in again."),
    ///     Err(TokenError::InvalidSignature) => println!("Invalid token signature."),
    ///     Err(e) => println!("Token validation error: {:?}", e),
    /// }
    /// ```
    pub fn verify_token(&self, token: &str) -> Result<Claims, TokenError> {
        let validation = Validation::default();

        match decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &validation,
        ) {
            Ok(data) => Ok(data.claims),
            Err(err) => match *err.kind() {
                ErrorKind::ExpiredSignature => Err(TokenError::Expired),
                ErrorKind::InvalidSignature => Err(TokenError::InvalidSignature),
                ErrorKind::InvalidToken => Err(TokenError::InvalidToken),
                _ => Err(TokenError::Other(err.to_string())),
            },
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expired_token() {
        let manager = TokenManager::new("mysecret", -100); // 过期处理
        let token = manager.generate_token("user1").unwrap();

        match manager.verify_token(&token) {
            Ok(claims) => {
                assert_eq!(claims.id, "user1".to_string());
            }
            Err(TokenError::Expired) => {
                panic!("Token could be expired, please login again.");
            }
            Err(TokenError::InvalidSignature) => {
                panic!("Token invalid，it could be changed, please login again.");
            }
            Err(e) => {
                panic!("Token valid error: {:?}", e);
            }
        }
    }
}
