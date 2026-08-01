//! Long-lived MCP token revocation check.
//!
//! cobotsrv verifies JWTs locally (JWKS), so a deleted/rotated long-lived MCP token would
//! otherwise keep working until its `exp`. This client asks cobotusr "is this `jti`
//! revoked?" — mirroring [`crate::enforce::EnforceClient`] — with a bounded cache so
//! we don't add a round-trip to every MCP request.
//!
//! It **fails open**: an unreachable/erroring cobotusr is treated as "not revoked" so a
//! brief outage doesn't reject otherwise-valid MCP traffic. Revocation resumes the moment
//! cobotusr is reachable again. Unknown jtis (ordinary OIDC session tokens) are never in the
//! denylist and therefore always pass.
//!
//! Only definitive introspect answers are cached — error outcomes are never
//! stored, so a token revoked during a cobotusr outage is honored as soon as
//! cobotusr recovers instead of after the cache TTL expires.

use std::fmt;
use std::time::Duration;

use moka::future::Cache;
use tracing::warn;

pub struct RevocationClient {
    client: reqwest::Client,
    cobotusr_base: String,
    cache: Cache<String, bool>,
}

/// Introspect did not produce a definitive answer (network error, non-success
/// status, or unparseable body). Never cached; callers fail open.
#[derive(Debug, Clone)]
struct IntrospectError(String);

impl fmt::Display for IntrospectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for IntrospectError {}

impl RevocationClient {
    pub fn new(cobotusr_base: String, cache_ttl_secs: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(10_000)
            .time_to_live(Duration::from_secs(cache_ttl_secs.max(1)))
            .build();
        Self {
            client: reqwest::Client::new(),
            cobotusr_base,
            cache,
        }
    }

    async fn call_introspect(&self, jti: &str, auth_header: &str) -> Result<bool, IntrospectError> {
        let url = format!("{}/api/v1/oidc/mcp-tokens/introspect", self.cobotusr_base);
        let mut req = self
            .client
            .post(&url)
            .json(&serde_json::json!({ "jti": jti }))
            .timeout(Duration::from_secs(5));
        if !auth_header.is_empty() {
            req = req.header("Authorization", auth_header);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| IntrospectError(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(IntrospectError(format!(
                "introspect returned {}",
                resp.status()
            )));
        }
        let json = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| IntrospectError(e.to_string()))?;
        Ok(json
            .get("revoked")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Is this token's `jti` revoked? Cached with bounded capacity + TTL; fails open on error.
    /// `try_get_with` single-flights concurrent misses so one cobotusr call serves them all.
    pub async fn is_revoked(&self, jti: &str, auth_header: &str) -> bool {
        if jti.is_empty() {
            return false;
        }
        match self
            .cache
            .try_get_with(jti.to_string(), self.call_introspect(jti, auth_header))
            .await
        {
            Ok(revoked) => revoked,
            Err(e) => {
                warn!(jti, error = %e, "mcp revocation introspect failed; failing open");
                false
            }
        }
    }
}
