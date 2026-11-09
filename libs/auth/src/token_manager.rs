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
    /// use auth::token_manager::TokenManager;
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
    /// use auth::token_manager::TokenManager;
    ///
    /// let manager = TokenManager::new("mysecret", 3600);
    /// let token = manager.generate_token("user123").unwrap(); // String ID
    /// let token2 = manager.generate_token(42).unwrap();       // i32 ID
    /// assert!(!token.is_empty());
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
    /// use auth::token_manager::{TokenManager, TokenError};
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
    pub fn generate_id_token(
        &self,
        subject: &str,
        issuer: &str,
        audience: Option<&str>,
        nonce: Option<&str>,
        access_token: Option<&str>,
    ) -> JwtResult<String> {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.expire_seconds);
        let at_hash = compute_at_hash(access_token);
        let claims = IdTokenClaims {
            iss: issuer.to_string(),
            sub: subject.to_owned(),
            aud: audience.map(|s| s.to_string()),
            iat: now.timestamp() as u64,
            exp: exp.timestamp() as u64,
            nonce: nonce.map(|s| s.to_string()),
            at_hash,
            ..Default::default()
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
    /// Strict local verification uses only the configured RSA key and profile.
    pub fn verify_security_credential(
        &self,
        token: &crate::security::identity::SecretToken,
        expected: &crate::security::credential::CredentialExpectation,
        clock: &dyn crate::security::budget::Clock,
    ) -> Result<
        crate::security::credential::VerifiedCredential,
        crate::security::error::SecurityError,
    > {
        use crate::security::{
            credential::{access_key_id, verify_access_with_key},
            error::SecurityError,
        };
        let key = self
            .rsa_decoding_key
            .as_ref()
            .ok_or(SecurityError::DependencyUnavailable)?;
        if access_key_id(token)? != self.kid {
            return Err(SecurityError::InvalidCredential);
        }
        verify_access_with_key(token, expected, key, clock)
    }
    /// A signed reference may target retired credentials for revocation. It is
    /// deliberately not a VerifiedCredential or a usable authentication context.
    pub fn revocation_reference(
        &self,
        token: &crate::security::identity::SecretToken,
        issuer: &str,
    ) -> Result<crate::security::credential::CredentialRef, crate::security::error::SecurityError>
    {
        use crate::security::{
            credential::{CredentialRef, SecurityAccessClaims, access_key_id},
            error::SecurityError,
        };
        let key = self
            .rsa_decoding_key
            .as_ref()
            .ok_or(SecurityError::DependencyUnavailable)?;
        if access_key_id(token)? != self.kid {
            return Err(SecurityError::InvalidCredential);
        }
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[issuer]);
        validation.validate_aud = false;
        validation.validate_exp = false;
        validation.validate_nbf = false;
        let claims = decode::<SecurityAccessClaims>(token.expose_secret(), key, &validation)
            .map_err(|_| SecurityError::InvalidCredential)?
            .claims;
        if claims.iss != issuer {
            return Err(SecurityError::InvalidCredential);
        }
        CredentialRef::new(claims.iss, claims.jti)
    }
    pub fn sign_security_access(
        &self,
        claims: &crate::security::credential::SecurityAccessClaims,
    ) -> Result<crate::security::identity::SecretToken, crate::security::error::SecurityError> {
        use crate::security::{error::SecurityError, identity::SecretToken};
        let key = self
            .rsa_encoding_key
            .as_ref()
            .ok_or(SecurityError::DependencyUnavailable)?;
        let mut header = Header::new(Algorithm::RS256);
        header.typ = Some("at+jwt".into());
        header.kid = Some(self.kid.clone());
        SecretToken::new(
            encode(&header, claims, key).map_err(|_| SecurityError::DependencyUnavailable)?,
        )
    }

    /// Create from RSA PEM private key.
    pub fn from_rsa_pem(pem: &[u8], expire_seconds: i64) -> Result<Self, String> {
        let encoding_key = EncodingKey::from_rsa_pem(pem)
            .map_err(|e| format!("Failed to parse RSA private key: {}", e))?;

        // Extract public key DER for JWKS + a DecodingKey for self-verify
        use rsa::pkcs8::DecodePrivateKey;
        let private_key = rsa::PrivateKey::from_pkcs8_pem(
            std::str::from_utf8(pem).map_err(|e| format!("Invalid UTF-8 in PEM: {}", e))?,
        )
        .map_err(|e| format!("Failed to parse RSA key: {}", e))?;
        let public_key = rsa::PublicKey::from(&private_key);
        use rsa::pkcs1::EncodeRsaPublicKey;
        let pub_der = public_key
            .to_pkcs1_der()
            .map_err(|e| format!("Failed to encode public key: {}", e))?;

        use base64::Engine;
        use rsa::traits::PublicKeyParts;
        let n_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public_key.n_bytes());
        let e_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public_key.e_bytes());
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
    pub fn sign_id_token(
        &self,
        subject: &str,
        issuer: &str,
        audience: Option<&str>,
        nonce: Option<&str>,
        access_token: Option<&str>,
    ) -> JwtResult<String> {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.expire_seconds);
        let at_hash = compute_at_hash(access_token);
        let claims = IdTokenClaims {
            iss: issuer.to_string(),
            sub: subject.to_owned(),
            aud: audience.map(|s| s.to_string()),
            iat: now.timestamp() as u64,
            exp: exp.timestamp() as u64,
            nonce: nonce.map(|s| s.to_string()),
            at_hash,
            ..Default::default()
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

    /// Sign the managed identity profile using the caller's trusted timestamps.
    pub fn sign_identity_id_token(
        &self,
        claims: &IdTokenClaims,
    ) -> Result<crate::security::identity::SecretToken, crate::security::error::SecurityError> {
        use crate::security::{
            error::SecurityError,
            identity::{PrincipalKind, PrincipalRef, SecretToken},
        };
        PrincipalRef::new(&claims.iss, PrincipalKind::User, &claims.sub)?;
        let key = self
            .rsa_encoding_key
            .as_ref()
            .ok_or(SecurityError::DependencyUnavailable)?;
        if claims.aud.as_ref().is_none_or(|aud| aud.is_empty())
            || claims.exp <= claims.iat
            || claims.auth_time.is_none_or(|at| at > claims.iat)
            || claims
                .sid
                .as_ref()
                .is_none_or(|sid| uuid::Uuid::parse_str(sid).is_err())
            || claims.email.is_none() && claims.email_verified.is_some()
        {
            return Err(SecurityError::InvalidCredential);
        }
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.kid.clone());
        SecretToken::new(
            encode(&header, claims, key).map_err(|_| SecurityError::DependencyUnavailable)?,
        )
    }

    /// Verify a local ID Token for logout association. Expiration is deliberately
    /// ignored; the caller must check the referenced current or recent RP session.
    pub fn logout_reference(
        &self,
        token: &crate::security::identity::SecretToken,
        issuer: &str,
        now_seconds: u64,
    ) -> Result<crate::claims::LogoutReference, crate::security::error::SecurityError> {
        use crate::security::{credential::restricted_jws_key_id, error::SecurityError};
        if token.expose_secret().len() > 8192
            || restricted_jws_key_id(token.expose_secret(), "JWT")? != self.kid
        {
            return Err(SecurityError::InvalidCredential);
        }
        let key = self
            .rsa_decoding_key
            .as_ref()
            .ok_or(SecurityError::DependencyUnavailable)?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[issuer]);
        validation.validate_aud = false;
        validation.validate_exp = false;
        let claims = decode::<IdTokenClaims>(token.expose_secret(), key, &validation)
            .map_err(|_| SecurityError::InvalidCredential)?
            .claims;
        let client_id = claims
            .aud
            .filter(|s| !s.is_empty())
            .ok_or(SecurityError::InvalidCredential)?;
        let protocol_sid = claims
            .sid
            .and_then(|s| uuid::Uuid::parse_str(&s).ok())
            .filter(|id| !id.is_nil())
            .ok_or(SecurityError::InvalidCredential)?;
        let auth_time = claims
            .auth_time
            .filter(|at| *at <= claims.iat)
            .ok_or(SecurityError::InvalidCredential)?;
        if claims.iss != issuer
            || claims.sub.is_empty()
            || claims.exp <= claims.iat
            || claims.iat > now_seconds.saturating_add(60)
        {
            return Err(SecurityError::InvalidCredential);
        }
        Ok(crate::claims::LogoutReference {
            issuer: claims.iss,
            subject: claims.sub,
            client_id,
            protocol_sid,
            auth_time,
        })
    }

    pub fn sign_logout_token(
        &self,
        claims: &crate::claims::LogoutTokenClaims,
    ) -> Result<crate::security::identity::SecretToken, crate::security::error::SecurityError> {
        use crate::security::{
            error::SecurityError,
            identity::{PrincipalKind, PrincipalRef, SecretToken},
        };
        PrincipalRef::new(&claims.iss, PrincipalKind::User, &claims.sub)?;
        let key = self
            .rsa_encoding_key
            .as_ref()
            .ok_or(SecurityError::DependencyUnavailable)?;
        if claims.aud.is_empty()
            || claims.exp <= claims.iat
            || claims.exp - claims.iat > 120
            || uuid::Uuid::parse_str(&claims.jti).is_err()
            || uuid::Uuid::parse_str(&claims.sid).is_err()
            || claims.events.len() != 1
            || claims
                .events
                .get("http://schemas.openid.net/event/backchannel-logout")
                .is_none_or(|v| v.as_object().is_none_or(|m| !m.is_empty()))
        {
            return Err(SecurityError::InvalidCredential);
        }
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.kid.clone());
        header.typ = Some("logout+jwt".into());
        SecretToken::new(
            encode(&header, claims, key).map_err(|_| SecurityError::DependencyUnavailable)?,
        )
    }

    /// Sign a standard JWT access token with RS256 (or HS256 fallback).
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
    /// Used by resource-app to verify its own tokens locally (no JWKS round-trip).
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
        let public_key = rsa::PublicKey::from_pkcs1_der(pub_der).ok()?;
        use rsa::traits::PublicKeyParts;
        let n_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public_key.n_bytes());
        let e_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public_key.e_bytes());

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

    fn identity_signer() -> IdTokenSigner {
        use rsa::pkcs8::{EncodePrivateKey, LineEnding};
        let key = rsa::PrivateKey::new(&mut getrandom::SysRng, 2048).unwrap();
        IdTokenSigner::from_rsa_pem(key.to_pkcs8_pem(LineEnding::LF).unwrap().as_bytes(), 300)
            .unwrap()
    }

    fn identity_claims() -> IdTokenClaims {
        IdTokenClaims {
            iss: "https://issuer.test".into(),
            sub: "123".into(),
            aud: Some("test-client".into()),
            iat: 1_000,
            exp: 1_300,
            auth_time: Some(900),
            sid: Some(Uuid::new_v4().to_string()),
            nonce: Some("original-nonce".into()),
            at_hash: Some("access-hash".into()),
            ..Default::default()
        }
    }

    fn verify_identity_json(signer: &IdTokenSigner, token: &str) -> serde_json::Value {
        let jwks: jsonwebtoken::jwk::JwkSet =
            serde_json::from_str(&signer.jwks_json().unwrap()).unwrap();
        let header = jsonwebtoken::decode_header(token).unwrap();
        assert_eq!(header.alg, Algorithm::RS256);
        let jwk = jwks.find(header.kid.as_deref().unwrap()).unwrap();
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&["https://issuer.test"]);
        validation.set_audience(&["test-client"]);
        // Fixed trusted times are intentional; this test is independent of wall time.
        validation.validate_exp = false;
        decode::<serde_json::Value>(token, &DecodingKey::from_jwk(jwk).unwrap(), &validation)
            .unwrap()
            .claims
    }

    #[test]
    fn identity_id_token_uses_published_rsa_key_and_omits_absent_profile() {
        let signer = identity_signer();
        let claims = identity_claims();
        let token = signer.sign_identity_id_token(&claims).unwrap();
        let actual = verify_identity_json(&signer, token.expose_secret());
        assert_eq!(actual["auth_time"], 900);
        assert_eq!(actual["iat"], 1_000);
        assert_eq!(actual["exp"], 1_300);
        assert_eq!(actual["sid"], claims.sid.unwrap());
        assert_eq!(actual["nonce"], "original-nonce");
        for field in ["name", "preferred_username", "email", "email_verified"] {
            assert!(
                actual.get(field).is_none(),
                "invented profile field: {field}"
            );
        }
        let mut bad_audience = Validation::new(Algorithm::RS256);
        bad_audience.validate_exp = false;
        bad_audience.set_audience(&["other-client"]);
        assert!(
            decode::<serde_json::Value>(
                token.expose_secret(),
                signer.rsa_decoding_key.as_ref().unwrap(),
                &bad_audience
            )
            .is_err()
        );
    }

    #[test]
    fn identity_signing_rejects_hmac_and_incomplete_authentication_context() {
        use crate::security::error::SecurityError;
        let claims = identity_claims();
        assert_eq!(
            IdTokenSigner::from_hmac("secret", 300)
                .sign_identity_id_token(&claims)
                .err(),
            Some(SecurityError::DependencyUnavailable)
        );
        let signer = identity_signer();
        for variant in 0..5 {
            let mut invalid = claims.clone();
            match variant {
                0 => invalid.auth_time = None,
                1 => invalid.auth_time = Some(invalid.iat + 1),
                2 => invalid.sid = Some("not-a-sid".into()),
                3 => invalid.exp = invalid.iat,
                _ => invalid.email_verified = Some(true),
            }
            assert_eq!(
                signer.sign_identity_id_token(&invalid).err(),
                Some(SecurityError::InvalidCredential)
            );
        }
    }

    #[test]
    fn identity_logout_reference_accepts_expired_id_token_but_rejects_other_key_and_profile() {
        use crate::security::{error::SecurityError, identity::SecretToken};
        let signer = identity_signer();
        let claims = identity_claims();
        let token = signer.sign_identity_id_token(&claims).unwrap();
        let reference = signer
            .logout_reference(&token, "https://issuer.test", 2000)
            .unwrap();
        assert_eq!(reference.client_id, "test-client");
        assert_eq!(reference.auth_time, 900);
        assert_eq!(reference.subject, "123");
        assert_eq!(
            signer
                .logout_reference(&token, "https://other.test", 2000)
                .err(),
            Some(SecurityError::InvalidCredential)
        );
        assert_eq!(
            signer
                .logout_reference(&token, "https://issuer.test", 900)
                .err(),
            Some(SecurityError::InvalidCredential)
        );
        let foreign = identity_signer().sign_identity_id_token(&claims).unwrap();
        assert_eq!(
            signer
                .logout_reference(&foreign, "https://issuer.test", 2000)
                .err(),
            Some(SecurityError::InvalidCredential)
        );
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(signer.kid.clone());
        header.typ = Some("at+jwt".into());
        let access_profile = SecretToken::new(
            encode(&header, &claims, signer.rsa_encoding_key.as_ref().unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            signer
                .logout_reference(&access_profile, "https://issuer.test", 2000)
                .err(),
            Some(SecurityError::InvalidCredential)
        );
        // A malicious JOSE URL/extension must not cause remote key retrieval.
        header.typ = Some("JWT".into());
        header.jku = Some("https://attacker.test/keys".into());
        let external_key = SecretToken::new(
            encode(&header, &claims, signer.rsa_encoding_key.as_ref().unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            signer
                .logout_reference(&external_key, "https://issuer.test", 2000)
                .err(),
            Some(SecurityError::InvalidCredential)
        );
    }

    #[test]
    fn identity_logout_token_has_single_event_sid_and_no_nonce() {
        use crate::{claims::LogoutTokenClaims, security::error::SecurityError};
        let signer = identity_signer();
        let claims = LogoutTokenClaims {
            iss: "https://issuer.test".into(),
            aud: "test-client".into(),
            sub: "123".into(),
            iat: 1_000,
            exp: 1_120,
            jti: Uuid::new_v4().to_string(),
            sid: Uuid::new_v4().to_string(),
            events: std::collections::BTreeMap::from([(
                "http://schemas.openid.net/event/backchannel-logout".into(),
                serde_json::json!({}),
            )]),
        };
        let token = signer.sign_logout_token(&claims).unwrap();
        let actual = verify_identity_json(&signer, token.expose_secret());
        assert_eq!(actual["sid"], claims.sid);
        assert_eq!(actual["jti"], claims.jti);
        assert!(actual.get("nonce").is_none());
        assert_eq!(actual["events"].as_object().unwrap().len(), 1);
        let mut invalid = claims.clone();
        invalid.exp += 1;
        assert_eq!(
            signer.sign_logout_token(&invalid).err(),
            Some(SecurityError::InvalidCredential)
        );
        invalid = claims;
        invalid
            .events
            .insert("unexpected".into(), serde_json::json!({}));
        assert_eq!(
            signer.sign_logout_token(&invalid).err(),
            Some(SecurityError::InvalidCredential)
        );
        let mut with_nonce = actual;
        with_nonce["nonce"] = serde_json::json!("forbidden");
        assert!(serde_json::from_value::<LogoutTokenClaims>(with_nonce).is_err());
    }

    fn assert_id_token_subject(token: &str, key: &DecodingKey, algorithm: Algorithm) {
        let mut validation = Validation::new(algorithm);
        validation.set_issuer(&["https://issuer.test"]);
        validation.set_audience(&["test-client"]);
        let claims = decode::<serde_json::Value>(token, key, &validation)
            .expect("ID Token signature and claims should be valid")
            .claims;
        assert_eq!(claims["sub"], serde_json::json!("123"));
    }

    #[test]
    fn test_generate_id_token_hmac_string_subject() {
        let manager = TokenManager::new("test-secret", 300);
        let token = manager
            .generate_id_token(
                &123.to_string(),
                "https://issuer.test",
                Some("test-client"),
                None,
                None,
            )
            .unwrap();
        assert_id_token_subject(
            &token,
            &DecodingKey::from_secret(b"test-secret"),
            Algorithm::HS256,
        );
    }

    #[test]
    fn test_sign_id_token_hmac_string_subject() {
        let signer = IdTokenSigner::from_hmac("test-secret", 300);
        let token = signer
            .sign_id_token(
                &123.to_string(),
                "https://issuer.test",
                Some("test-client"),
                None,
                None,
            )
            .unwrap();
        assert_id_token_subject(
            &token,
            &DecodingKey::from_secret(b"test-secret"),
            Algorithm::HS256,
        );
    }

    #[test]
    fn test_sign_id_token_rsa_string_subject() {
        use rsa::pkcs8::{EncodePrivateKey, LineEnding};

        let private_key = rsa::PrivateKey::new(&mut getrandom::SysRng, 2048).unwrap();
        let pem = private_key.to_pkcs8_pem(LineEnding::LF).unwrap();
        let signer = IdTokenSigner::from_rsa_pem(pem.as_bytes(), 300).unwrap();
        let token = signer
            .sign_id_token(
                &123.to_string(),
                "https://issuer.test",
                Some("test-client"),
                None,
                None,
            )
            .unwrap();
        assert_id_token_subject(
            &token,
            signer.rsa_decoding_key.as_ref().unwrap(),
            Algorithm::RS256,
        );
    }

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
