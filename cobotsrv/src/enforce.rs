use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use auth::PermissionChecker;
use dashmap::DashMap;
use tracing::{debug, warn};

/// Client that calls cobotusr's PDP `/api/v1/enforce` endpoint with local
/// caching to avoid a network round-trip on every request.
pub struct EnforceClient {
    client: reqwest::Client,
    cobotusr_base: String,
    /// Application key to enforce against — must match the app this service
    /// registered its permissions under (its OAuth `client_id`).
    application: String,
    cache: DashMap<(String, String), (bool, Instant)>,
    cache_ttl: Duration,
}

impl EnforceClient {
    pub fn new(cobotusr_base: String, application: String, cache_ttl_secs: u64) -> Self {
        Self {
            client: reqwest::Client::new(),
            cobotusr_base,
            application,
            cache: DashMap::new(),
            cache_ttl: Duration::from_secs(cache_ttl_secs.max(1)),
        }
    }

    async fn call_pdp(&self, subject: &str, action: &str, auth_header: &str) -> bool {
        let url = format!("{}/api/v1/enforce", self.cobotusr_base);

        let subject_id = subject.parse::<i32>().unwrap_or_else(|_| {
            warn!(subject, "cannot parse subject as i32, defaulting to 0");
            0
        });
        let body = serde_json::json!({
            "application": self.application,
            "subject": subject_id,
            "action": action,
        });

        let mut req = self
            .client
            .post(&url)
            .json(&body)
            .timeout(Duration::from_secs(5));
        if !auth_header.is_empty() {
            req = req.header("Authorization", auth_header);
        }

        match req.send().await {
            Ok(resp) => match resp.json::<serde_json::Value>().await {
                Ok(json) => {
                    let allowed = json.get("allow").and_then(|v| v.as_bool()).unwrap_or(false);
                    debug!(subject, action, allowed, "pdp enforce result");
                    allowed
                }
                Err(e) => {
                    warn!(subject, action, error = %e, "pdp response parse failed");
                    false
                }
            },
            Err(e) => {
                warn!(subject, action, error = %e, "pdp request failed");
                false
            }
        }
    }

    pub async fn check(&self, subject: &str, permission: &str, auth_header: &str) -> bool {
        let cache_key = (subject.to_string(), permission.to_string());

        // Check cache
        if let Some(entry) = self.cache.get(&cache_key)
            && entry.value().1.elapsed() < self.cache_ttl
        {
            return entry.value().0;
        }

        let allowed = self.call_pdp(subject, permission, auth_header).await;
        self.cache.insert(cache_key, (allowed, Instant::now()));
        allowed
    }
}

impl PermissionChecker for EnforceClient {
    fn check_permission<'a>(
        &'a self,
        subject: &str,
        permission: &str,
        auth_header: &str,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        let sub = subject.to_string();
        let perm = permission.to_string();
        let auth = auth_header.to_string();
        Box::pin(async move { self.check(&sub, &perm, &auth).await })
    }
}
