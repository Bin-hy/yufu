use crate::claims::Claims;

use chrono::{Duration, Utc};
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{
    DecodingKey, EncodingKey, Header, Validation, decode, encode, errors::Result as JwtResult,
};
use serde::{Deserialize, Serialize};

/// Token管理器，用于统一生成与校验
#[derive(Clone)]
pub struct TokenManager {
    secret: String,
    expire_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TokenError {
    Expired,
    InvalidSignature,
    InvalidToken,
    Other(String),
}

impl TokenManager {
    /// create a new TokenManager
    pub fn new(secret: impl Into<String>, expire_seconds: i64) -> Self {
        Self {
            secret: secret.into(),
            expire_seconds,
        }
    }

    /// creat a JWT token
    pub fn generate_token(&self, user_id: &str) -> JwtResult<String> {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.expire_seconds);
        let claims = Claims {
            id: user_id.to_string(),
            iat: now.timestamp() as u64,
            exp: exp.timestamp() as u64,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
    }

    /// Validate token, return Claims
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
                println!("✅ Token valid pass, id: {}", claims.id);
            }
            Err(TokenError::Expired) => {
                println!("⚠️ Token could be expired, please login again.");
            }
            Err(TokenError::InvalidSignature) => {
                println!("🚫 Token invalid，it could be changed, please login again.");
            }
            Err(e) => {
                println!("❌ Token valid error: {:?}", e);
            }
        }
    }
}
