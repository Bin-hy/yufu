use crate::claims::{AccessTokenClaims, Claims, IdTokenClaims, RefreshClaims};

use chrono::{Duration, Utc};
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode,
    errors::Result as JwtResult,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

    /// Generates a new JWT token for a given user ID.
    ///
    /// Supports generic ID types for flexibility.
    ///
    /// # Arguments
    ///
    /// * `user_id` - The ID of the user for whom the token will be generated.
    ///
    /// # Returns
    ///
    /// Returns a signed JWT token string if successful (`JwtResult<String>`).
    ///
    /// # Example
    ///
    /// ```
    /// use auth::TokenManager;
    ///
    /// let manager = TokenManager::new("mysecret", 3600);
    /// let token = manager.generate_token("user123").unwrap(); // String ID
    /// let token2 = manager.generate_token(42).unwrap();       // i32 ID
    /// println!("Generated token: {}", token);
    /// ```
    pub fn generate_token<IDTYPE>(&self, user_id: IDTYPE) -> JwtResult<String>
    where
        IDTYPE: Clone + Send + Sync + 'static + Serialize,
    {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.expire_seconds);
        let claims = Claims {
            id: user_id,
            exp: exp.timestamp() as u64,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
    }

    pub fn generate_refresh_token<IDTYPE>(&self, jti: String, user_id: IDTYPE) -> JwtResult<String>
    where
        IDTYPE: Clone + Send + Sync + 'static + Serialize,
    {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.expire_seconds);
        let claims = RefreshClaims {
            jti,
            sub: user_id,
            exp: exp.timestamp() as u64,
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
    }

    /// Validates a JWT token and returns the decoded [`Claims<IDTYPE>`] if valid.
    ///
    /// Supports generic ID types for flexibility.
    ///
    /// # Arguments
    ///
    /// * `token` - The JWT token string to verify.
    ///
    /// # Returns
    ///
    /// Returns `Claims<IDTYPE>` on success, or a `TokenError` on failure.
    ///
    /// # Errors
    ///
    /// - `TokenError::Expired` - Token has expired.
    /// - `TokenError::InvalidSignature` - Signature is invalid.
    /// - `TokenError::InvalidToken` - Token format or content is invalid.
    /// - `TokenError::Other(String)` - Any other decoding error.
    ///
    /// # Example
    ///
    /// ```
    /// use auth::{TokenManager, TokenError, Claims};
    ///
    /// let manager = TokenManager::new("mysecret", 3600);
    /// let token = manager.generate_token::<String>("user1".to_string()).unwrap();
    ///
    /// match manager.verify_token::<String>(&token) {
    ///     Ok(claims) => println!("Token valid, id: {}", claims.id),
    ///     Err(TokenError::Expired) => println!("Token expired, please log in again."),
    ///     Err(TokenError::InvalidSignature) => println!("Invalid token signature."),
    ///     Err(e) => println!("Token validation error: {:?}", e),
    /// }
    /// ```
    pub fn verify_token<IDTYPE>(&self, token: &str) -> Result<Claims<IDTYPE>, TokenError>
    where
        for<'de> IDTYPE: Clone + Send + Sync + 'static + Serialize + Deserialize<'de>,
    {
        let validation = Validation::default();

        match decode::<Claims<IDTYPE>>(
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

    pub fn verify_refresh_token<IDTYPE>(
        &self,
        token: &str,
    ) -> Result<RefreshClaims<IDTYPE>, TokenError>
    where
        for<'de> IDTYPE: Clone + Send + Sync + 'static + Serialize + Deserialize<'de>,
    {
        let validation = Validation::default();
        match decode::<RefreshClaims<IDTYPE>>(
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

    /// Generates an OIDC ID Token (HMAC fallback, prefer IdTokenSigner for RSA).
    pub fn generate_id_token<IDTYPE>(
        &self,
        subject: IDTYPE,
        issuer: &str,
        audience: Option<&str>,
        nonce: Option<&str>,
        access_token: Option<&str>,
    ) -> JwtResult<String>
    where
        IDTYPE: Clone + Send + Sync + 'static + Serialize,
    {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.expire_seconds);
        let at_hash = compute_at_hash(access_token);
        let claims = IdTokenClaims {
            iss: issuer.to_string(),
            sub: subject,
            aud: audience.map(|s| s.to_string()),
            iat: now.timestamp() as u64,
            exp: exp.timestamp() as u64,
            nonce: nonce.map(|s| s.to_string()),
            at_hash,
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
    }
}

fn compute_at_hash(access_token: Option<&str>) -> Option<String> {
    access_token.map(|at| {
        use base64::Engine;
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(at.as_bytes());
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&hash[..16])
    })
}

/// RSA-based ID Token signer for OIDC.
/// Falls back to HMAC if no RSA key is provided.
#[derive(Clone)]
pub struct IdTokenSigner {
    rsa_encoding_key: Option<EncodingKey>,
    rsa_decoding_key: Option<DecodingKey>,
    rsa_public_key_der: Option<Vec<u8>>, // for JWKS
    hmac_secret: String,
    pub expire_seconds: i64,
    pub kid: String,
}

impl IdTokenSigner {
    /// Sign only the dedicated operation profile; never fall back to shared HMAC.
    pub fn sign_gateway_operation(
        &self,
        permit: &crate::security::message::GatewayOperationPermit,
    ) -> Result<crate::security::message::SignedDocument, crate::security::error::SecurityError>
    {
        let key = self
            .rsa_encoding_key
            .as_ref()
            .ok_or(crate::security::error::SecurityError::InvalidCredential)?;
        crate::security::message::sign_gateway_operation(permit, key, &self.kid)
    }

    /// Create from RSA PEM private key.
    pub fn from_rsa_pem(pem: &[u8], expire_seconds: i64) -> Result<Self, String> {
        let encoding_key = EncodingKey::from_rsa_pem(pem)
            .map_err(|e| format!("Failed to parse RSA private key: {}", e))?;

        // Extract public key DER for JWKS + a DecodingKey for self-verify
        use rsa::pkcs8::DecodePrivateKey;
        let private_key = rsa::RsaPrivateKey::from_pkcs8_pem(
            std::str::from_utf8(pem).map_err(|e| format!("Invalid UTF-8 in PEM: {}", e))?,
        )
        .map_err(|e| format!("Failed to parse RSA key: {}", e))?;
        let public_key = rsa::RsaPublicKey::from(&private_key);
        use rsa::pkcs1::EncodeRsaPublicKey;
        let pub_der = public_key
            .to_pkcs1_der()
            .map_err(|e| format!("Failed to encode public key: {}", e))?;

        use base64::Engine;
        use rsa::traits::PublicKeyParts;
        let n_b64 =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be());
        let e_b64 =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be());
        let decoding_key = DecodingKey::from_rsa_components(&n_b64, &e_b64)
            .map_err(|e| format!("Failed to build RSA decoding key: {}", e))?;

        Ok(Self {
            rsa_encoding_key: Some(encoding_key),
            rsa_decoding_key: Some(decoding_key),
            rsa_public_key_der: Some(pub_der.as_bytes().to_vec()),
            hmac_secret: String::new(),
            expire_seconds,
            kid: "cobotrix-rsa-1".to_string(),
        })
    }

    /// Create HMAC-only fallback.
    pub fn from_hmac(secret: &str, expire_seconds: i64) -> Self {
        Self {
            rsa_encoding_key: None,
            rsa_decoding_key: None,
            rsa_public_key_der: None,
            hmac_secret: secret.to_string(),
            expire_seconds,
            kid: "cobotrix-hmac-1".to_string(),
        }
    }

    pub fn is_rsa(&self) -> bool {
        self.rsa_encoding_key.is_some()
    }

    pub fn algorithm(&self) -> &str {
        if self.is_rsa() { "RS256" } else { "HS256" }
    }

    /// Generate a signed ID Token.
    pub fn sign_id_token<IDTYPE>(
        &self,
        subject: IDTYPE,
        issuer: &str,
        audience: Option<&str>,
        nonce: Option<&str>,
        access_token: Option<&str>,
    ) -> JwtResult<String>
    where
        IDTYPE: Clone + Send + Sync + 'static + Serialize,
    {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.expire_seconds);
        let at_hash = compute_at_hash(access_token);
        let claims = IdTokenClaims {
            iss: issuer.to_string(),
            sub: subject,
            aud: audience.map(|s| s.to_string()),
            iat: now.timestamp() as u64,
            exp: exp.timestamp() as u64,
            nonce: nonce.map(|s| s.to_string()),
            at_hash,
        };

        if let Some(ref rsa_key) = self.rsa_encoding_key {
            let mut header = Header::new(Algorithm::RS256);
            header.kid = Some(self.kid.clone());
            encode(&header, &claims, rsa_key)
        } else {
            encode(
                &Header::default(),
                &claims,
                &EncodingKey::from_secret(self.hmac_secret.as_bytes()),
            )
        }
    }

    /// Sign a standard JWT access token (RFC 7519 claims) with RS256 (or HS256 fallback).
    ///
    /// `aud` is the application/client this token is intended for; consumers should verify it.
    /// `roles` and `permissions` are embedded so downstream services can make coarse-grained
    /// local decisions without calling the PDP on every request.
    /// Returns the encoded token. `jti` is generated as a fresh UUID v4.
    #[allow(clippy::too_many_arguments)]
    pub fn sign_access_token(
        &self,
        sub: &str,
        iss: &str,
        aud: &str,
        scope: &str,
        ttl_seconds: i64,
        roles: Vec<String>,
        permissions: Vec<String>,
    ) -> JwtResult<String> {
        self.sign_access_token_with_jti(
            sub,
            iss,
            aud,
            scope,
            ttl_seconds,
            roles,
            permissions,
            &Uuid::new_v4().to_string(),
        )
    }

    /// Like [`Self::sign_access_token`] but with a caller-supplied `jti`. Used when the
    /// issuer needs the token's id up front (e.g. to persist a revocable MCP-token record
    /// keyed by `jti` before handing the token out).
    #[allow(clippy::too_many_arguments)]
    pub fn sign_access_token_with_jti(
        &self,
        sub: &str,
        iss: &str,
        aud: &str,
        scope: &str,
        ttl_seconds: i64,
        roles: Vec<String>,
        permissions: Vec<String>,
        jti: &str,
    ) -> JwtResult<String> {
        let now = Utc::now();
        let exp = now + Duration::seconds(ttl_seconds);
        let claims = AccessTokenClaims {
            iss: iss.to_string(),
            sub: sub.to_string(),
            aud: aud.to_string(),
            iat: now.timestamp() as u64,
            exp: exp.timestamp() as u64,
            jti: jti.to_string(),
            scope: scope.to_string(),
            roles,
            permissions,
        };

        if let Some(ref rsa_key) = self.rsa_encoding_key {
            let mut header = Header::new(Algorithm::RS256);
            header.kid = Some(self.kid.clone());
            encode(&header, &claims, rsa_key)
        } else {
            encode(
                &Header::default(),
                &claims,
                &EncodingKey::from_secret(self.hmac_secret.as_bytes()),
            )
        }
    }

    /// Verify a JWT access token using this signer's own key.
    ///
    /// Used by cobotsrv to verify its own tokens locally (no JWKS round-trip).
    /// Other services should use [`crate::jwks::JwksClient`] instead.
    /// `expected_aud` — if Some, requires the token's `aud` claim to match exactly.
    pub fn verify_access_token(
        &self,
        token: &str,
        expected_aud: Option<&str>,
    ) -> Result<AccessTokenClaims, TokenError> {
        let mut validation = if self.is_rsa() {
            Validation::new(Algorithm::RS256)
        } else {
            Validation::new(Algorithm::HS256)
        };
        match expected_aud {
            Some(a) => validation.set_audience(&[a]),
            None => validation.validate_aud = false,
        }

        let key = if let Some(ref dk) = self.rsa_decoding_key {
            dk.clone()
        } else {
            DecodingKey::from_secret(self.hmac_secret.as_bytes())
        };

        match decode::<AccessTokenClaims>(token, &key, &validation) {
            Ok(data) => Ok(data.claims),
            Err(err) => match *err.kind() {
                ErrorKind::ExpiredSignature => Err(TokenError::Expired),
                ErrorKind::InvalidSignature => Err(TokenError::InvalidSignature),
                ErrorKind::InvalidToken => Err(TokenError::InvalidToken),
                _ => Err(TokenError::Other(err.to_string())),
            },
        }
    }

    /// Export JWKS JSON for the public key endpoint.
    /// Returns None if using HMAC (no public key to expose).
    pub fn jwks_json(&self) -> Option<String> {
        use base64::Engine;
        use rsa::pkcs1::DecodeRsaPublicKey;
        let pub_der = self.rsa_public_key_der.as_ref()?;

        // Parse the DER to extract n and e
        let public_key = rsa::RsaPublicKey::from_pkcs1_der(pub_der).ok()?;
        use rsa::traits::PublicKeyParts;
        let n = public_key.n();
        let e = public_key.e();

        let n_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(n.to_bytes_be());
        let e_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(e.to_bytes_be());

        let jwks = serde_json::json!({
            "keys": [{
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": self.kid,
                "n": n_b64,
                "e": e_b64,
            }]
        });
        Some(jwks.to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expired_token() {
        let manager = TokenManager::new("mysecret", 300); // 过期处理
        let token = manager.generate_token(1).unwrap();

        match manager.verify_token::<i32>(&token) {
            Ok(claims) => {
                assert_eq!(claims.id, 1);
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
