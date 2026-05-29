//! JWKS-based JWT verification for cross-service auth.
//!
//! Pulls a JSON Web Key Set from an IdP's `/.well-known/jwks.json` (or any
//! configured URL), caches the parsed [`DecodingKey`]s by `kid`, refreshes them
//! periodically in a background task, and exposes a `verify` method consumers
//! call per request.
//!
//! Unlike [`crate::token_manager::IdTokenSigner::verify_access_token`] which
//! requires holding the private key, this works with the public JWKS only —
//! the right tool for an OIDC client.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;

use crate::claims::AccessTokenClaims;
use crate::token_manager::TokenError;

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: String,
    n: String,
    e: String,
}

/// Client that fetches a JWKS, caches it, and verifies access tokens against it.
///
/// Cheap to clone (Arc inside).
#[derive(Clone)]
pub struct JwksClient {
    jwks_url: String,
    keys: Arc<RwLock<HashMap<String, DecodingKey>>>,
    expected_aud: Option<String>,
}

impl JwksClient {
    /// Create a client and fetch the JWKS once. Errors out if the initial fetch fails;
    /// callers should treat this as a hard startup failure (or wrap it in a retry loop).
    pub async fn new(
        jwks_url: impl Into<String>,
        expected_aud: Option<String>,
    ) -> anyhow::Result<Self> {
        let client = Self {
            jwks_url: jwks_url.into(),
            keys: Arc::new(RwLock::new(HashMap::new())),
            expected_aud,
        };
        client.refresh().await?;
        Ok(client)
    }

    /// Spawn a background task that refreshes the JWKS every `interval_secs` seconds.
    /// Failures are logged but don't crash the task — stale keys keep working until exp.
    pub fn spawn_refresher(self: &Arc<Self>, interval_secs: u64) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            tick.tick().await; // skip immediate fire (we just fetched in new())
            loop {
                tick.tick().await;
                if let Err(e) = this.refresh().await {
                    tracing::warn!(error = %e, jwks_url = %this.jwks_url, "JWKS refresh failed; using cached keys");
                }
            }
        });
    }

    async fn refresh(&self) -> anyhow::Result<()> {
        let resp: Jwks = reqwest::get(&self.jwks_url)
            .await?
            .error_for_status()?
            .json()
            .await?;
        let mut new_keys = HashMap::with_capacity(resp.keys.len());
        for jwk in resp.keys {
            match DecodingKey::from_rsa_components(&jwk.n, &jwk.e) {
                Ok(key) => {
                    new_keys.insert(jwk.kid, key);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "skipping malformed JWK");
                }
            }
        }
        if new_keys.is_empty() {
            anyhow::bail!("JWKS at {} returned no usable keys", self.jwks_url);
        }
        let count = new_keys.len();
        *self.keys.write().unwrap() = new_keys;
        tracing::info!(count, jwks_url = %self.jwks_url, "JWKS refreshed");
        Ok(())
    }

    /// Verify a JWT access token. Selects the public key by the token header's `kid`.
    /// Validates signature, exp, and (if configured) audience.
    pub fn verify(&self, token: &str) -> Result<AccessTokenClaims, TokenError> {
        let header = decode_header(token).map_err(|e| match *e.kind() {
            ErrorKind::InvalidToken => TokenError::InvalidToken,
            _ => TokenError::Other(e.to_string()),
        })?;
        let kid = header.kid.ok_or(TokenError::InvalidToken)?;
        let key = {
            let map = self.keys.read().unwrap();
            map.get(&kid).cloned().ok_or(TokenError::InvalidSignature)?
        };

        let mut validation = Validation::new(Algorithm::RS256);
        match &self.expected_aud {
            Some(a) => validation.set_audience(&[a]),
            None => validation.validate_aud = false,
        }

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
}
