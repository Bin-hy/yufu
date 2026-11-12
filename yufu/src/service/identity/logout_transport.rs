//! HTTPS targets are resolved once, checked in full, then pinned for the request.
use super::{logout_dispatcher::LogoutTransport, repository_outbox::DeliveryOutcome};
use crate::config_identity::Identity;
use auth::security::{budget::Clock, identity::SecretToken};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

#[async_trait::async_trait]
#[allow(clippy::double_must_use)]
pub trait LogoutResolver: Send + Sync {
    async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, ()>;
}
pub struct SystemLogoutResolver;
#[async_trait::async_trait]
impl LogoutResolver for SystemLogoutResolver {
    async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, ()> {
        tokio::net::lookup_host((host, port))
            .await
            .map(|addresses| addresses.collect())
            .map_err(|_| ())
    }
}
pub struct HttpsLogoutTransport {
    policy: Identity,
    resolver: Arc<dyn LogoutResolver>,
    clock: Arc<dyn Clock>,
    extra_roots: Vec<reqwest::Certificate>,
    loopback_fixture: bool,
}
impl HttpsLogoutTransport {
    pub fn new(policy: Identity, clock: Arc<dyn Clock>) -> Self {
        Self::with_resolver(policy, clock, Arc::new(SystemLogoutResolver))
    }
    pub fn with_resolver(
        policy: Identity,
        clock: Arc<dyn Clock>,
        resolver: Arc<dyn LogoutResolver>,
    ) -> Self {
        Self {
            policy,
            resolver,
            clock,
            extra_roots: Vec::new(),
            loopback_fixture: false,
        }
    }
    /// Trusts additional certificate authorities for the delivery client.
    ///
    /// Production keeps the bundled roots only. Tests use this to point the
    /// transport at a local TLS receiver whose certificate a private CA issued,
    /// so the success path is exercised over real HTTPS rather than a mock.
    pub fn with_extra_roots(mut self, roots: Vec<reqwest::Certificate>) -> Self {
        self.extra_roots = roots;
        self
    }
    /// Lets the delivery client reach loopback addresses.
    ///
    /// Only an isolated test policy may do this. Production deployments keep
    /// loopback unreachable so a registered endpoint cannot turn into a request
    /// against a service on the identity host.
    pub fn with_loopback_fixture(mut self) -> Self {
        self.loopback_fixture = true;
        self
    }
    async fn send_checked(&self, endpoint: &str, token: &SecretToken) -> DeliveryOutcome {
        let uri = match url::Url::parse(endpoint) {
            Ok(uri)
                if uri.scheme() == "https"
                    && uri.host_str().is_some()
                    && uri.username().is_empty()
                    && uri.password().is_none()
                    && uri.fragment().is_none()
                    && uri.port_or_known_default() != Some(0) =>
            {
                uri
            }
            _ => {
                return DeliveryOutcome::PermanentFailure {
                    error_code: "invalid_endpoint",
                };
            }
        };
        let host = match uri.host() {
            Some(url::Host::Domain(host)) => host.to_owned(),
            Some(url::Host::Ipv4(ip)) => ip.to_string(),
            Some(url::Host::Ipv6(ip)) => ip.to_string(),
            None => {
                return DeliveryOutcome::PermanentFailure {
                    error_code: "invalid_endpoint",
                };
            }
        };
        let port = uri.port_or_known_default().unwrap_or(443);
        let addresses = match self.resolver.resolve(&host, port).await {
            Ok(addresses) if !addresses.is_empty() && addresses.len() <= 64 => addresses,
            _ => {
                return DeliveryOutcome::Retry {
                    error_code: "network_error",
                };
            }
        };
        let private = self
            .policy
            .logout_private_origins
            .contains(&uri.origin().ascii_serialization());
        if addresses.iter().any(|address| {
            address.port() != port || !allowed_ip(address.ip(), private, self.loopback_fixture)
        }) {
            return DeliveryOutcome::PermanentFailure {
                error_code: "ssrf_blocked",
            };
        }
        // A fresh client has no connection pool from a prior DNS answer. Neither
        // redirects, proxy environment variables nor a second lookup can change it.
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .https_only(true)
            .http1_only()
            .resolve_to_addrs(&host, &addresses)
            .timeout(Duration::from_millis(self.policy.logout_timeout_millis));
        for root in &self.extra_roots {
            builder = builder.add_root_certificate(root.clone());
        }
        let client = match builder.build() {
            Ok(client) => client,
            Err(_) => {
                return DeliveryOutcome::Retry {
                    error_code: "network_error",
                };
            }
        };
        let response = match client
            .post(uri)
            .form(&[("logout_token", token.expose_secret())])
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                return DeliveryOutcome::Retry {
                    error_code: if error.is_timeout() {
                        "timeout"
                    } else {
                        "network_error"
                    },
                };
            }
        };
        let at = match crate::service::security::repository::now(self.clock.as_ref()) {
            Ok(at) => at / 1000,
            Err(_) => {
                return DeliveryOutcome::Retry {
                    error_code: "clock_unavailable",
                };
            }
        };
        let after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| retry_after(s, at));
        http_outcome(response.status().as_u16(), after)
    }
}
#[async_trait::async_trait]
impl LogoutTransport for HttpsLogoutTransport {
    async fn send(&self, endpoint: &str, token: &SecretToken) -> DeliveryOutcome {
        match tokio::time::timeout(
            Duration::from_millis(self.policy.logout_timeout_millis),
            self.send_checked(endpoint, token),
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(_) => DeliveryOutcome::Retry {
                error_code: "timeout",
            },
        }
    }
}

fn retry_after(value: &str, now_seconds: u64) -> Option<u64> {
    if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
        return value.parse::<u64>().ok().map(|s| s.min(86_400));
    }
    let date = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    u64::try_from(date.timestamp())
        .ok()
        .map(|at| at.saturating_sub(now_seconds).min(86_400))
}
fn http_outcome(status: u16, retry_after: Option<u64>) -> DeliveryOutcome {
    let code = match status {
        200..=299 => return DeliveryOutcome::Delivered,
        300..=399 => {
            return DeliveryOutcome::PermanentFailure {
                error_code: "http_redirect",
            };
        }
        429 => "http_429",
        500..=599 => "http_5xx",
        _ => {
            return DeliveryOutcome::PermanentFailure {
                error_code: "http_4xx",
            };
        }
    };
    match retry_after {
        Some(seconds) => DeliveryOutcome::RetryAfter {
            error_code: code,
            retry_after_seconds: seconds,
        },
        None => DeliveryOutcome::Retry { error_code: code },
    }
}

fn allowed_ip(ip: IpAddr, private_allowed: bool, loopback_allowed: bool) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, d] = ip.octets();
            if a == 0
                || (a == 127 && !loopback_allowed)
                || a >= 224
                || (a == 169 && b == 254)
                || (a == 100 && b == 100 && c == 100 && d == 200)
                || (a == 192 && b == 0 && (c == 0 || c == 2))
                || (a == 198 && (b == 18 || b == 19))
                || (a == 198 && b == 51 && c == 100)
                || (a == 203 && b == 0 && c == 113)
                || (a == 192 && b == 88 && c == 99)
            {
                return false;
            }
            let private = ip.is_private() || (a == 100 && (64..=127).contains(&b));
            !private || private_allowed
        }
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return allowed_ip(IpAddr::V4(mapped), private_allowed, loopback_allowed);
            }
            if ip.is_loopback() {
                return loopback_allowed;
            }
            let s = ip.segments();
            if s[0] & 0xfe00 == 0xfc00 {
                return private_allowed;
            }
            // Only public unicast; reject link-local, documentation and tunnel ranges.
            s[0] & 0xe000 == 0x2000
                && !(s[0] == 0x2001 && matches!(s[1], 0 | 2 | 0xdb8))
                && s[0] != 0x2002
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct FixedResolver(Vec<SocketAddr>);
    #[async_trait::async_trait]
    impl LogoutResolver for FixedResolver {
        async fn resolve(&self, _host: &str, _port: u16) -> Result<Vec<SocketAddr>, ()> {
            Ok(self.0.clone())
        }
    }
    struct SlowResolver;
    #[async_trait::async_trait]
    impl LogoutResolver for SlowResolver {
        async fn resolve(&self, _host: &str, _port: u16) -> Result<Vec<SocketAddr>, ()> {
            tokio::time::sleep(Duration::from_millis(60)).await;
            Ok(vec!["8.8.8.8:443".parse().unwrap()])
        }
    }
    #[tokio::test]
    async fn logout_resolution_checks_every_address_and_timeout_includes_dns() {
        let clock: Arc<dyn Clock> =
            Arc::new(auth::security::budget::SystemClock::new(Arc::new(|| true)));
        let token = SecretToken::new("test-token".into()).unwrap();
        let policy = Identity {
            logout_timeout_millis: 20,
            logout_private_origins: vec!["https://rp.example".into()],
            ..Default::default()
        };
        let mixed = HttpsLogoutTransport::with_resolver(
            policy.clone(),
            clock.clone(),
            Arc::new(FixedResolver(vec![
                "8.8.8.8:443".parse().unwrap(),
                "127.0.0.1:443".parse().unwrap(),
            ])),
        );
        assert_eq!(
            mixed.send("https://rp.example/logout", &token).await,
            DeliveryOutcome::PermanentFailure {
                error_code: "ssrf_blocked"
            }
        );
        assert_eq!(
            mixed.send("http://rp.example/logout", &token).await,
            DeliveryOutcome::PermanentFailure {
                error_code: "invalid_endpoint"
            }
        );
        let private = HttpsLogoutTransport::with_resolver(
            policy.clone(),
            clock.clone(),
            Arc::new(FixedResolver(vec!["10.1.2.3:443".parse().unwrap()])),
        );
        assert_eq!(
            private.send("https://other.example/logout", &token).await,
            DeliveryOutcome::PermanentFailure {
                error_code: "ssrf_blocked"
            }
        );
        let slow = HttpsLogoutTransport::with_resolver(policy, clock, Arc::new(SlowResolver));
        assert_eq!(
            slow.send("https://rp.example/logout", &token).await,
            DeliveryOutcome::Retry {
                error_code: "timeout"
            }
        );
    }
    #[test]
    fn logout_delivery_status_and_retry_after_are_bounded() {
        assert_eq!(http_outcome(204, None), DeliveryOutcome::Delivered);
        assert_eq!(
            http_outcome(302, None),
            DeliveryOutcome::PermanentFailure {
                error_code: "http_redirect"
            }
        );
        assert_eq!(
            http_outcome(401, None),
            DeliveryOutcome::PermanentFailure {
                error_code: "http_4xx"
            }
        );
        assert_eq!(
            http_outcome(429, Some(60)),
            DeliveryOutcome::RetryAfter {
                error_code: "http_429",
                retry_after_seconds: 60
            }
        );
        assert_eq!(
            http_outcome(503, None),
            DeliveryOutcome::Retry {
                error_code: "http_5xx"
            }
        );
        assert_eq!(retry_after("60", 0), Some(60));
        assert_eq!(retry_after("999999999", 0), Some(86_400));
        assert_eq!(
            retry_after("Mon, 09 Nov 2026 00:00:00 GMT", 0),
            Some(86_400)
        );
        assert_eq!(retry_after("garbage", 0), None);
    }
    #[test]
    fn logout_network_policy_rejects_metadata_loopback_mapped_and_special_addresses() {
        for ip in [
            "0.0.0.0",
            "127.0.0.1",
            "169.254.169.254",
            "100.100.100.200",
            "224.0.0.1",
            "192.0.2.1",
            "198.18.0.1",
            "203.0.113.1",
            "::",
            "::1",
            "fe80::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "2001:db8::1",
            "2002:7f00:1::1",
        ] {
            assert!(!allowed_ip(ip.parse().unwrap(), false, false), "{ip}");
            assert!(!allowed_ip(ip.parse().unwrap(), true, false), "{ip}");
        }
        for ip in [
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "100.64.0.1",
            "fd00::1",
            "::ffff:10.1.2.3",
        ] {
            assert!(!allowed_ip(ip.parse().unwrap(), false, false), "{ip}");
            assert!(allowed_ip(ip.parse().unwrap(), true, false), "{ip}");
        }
        for ip in ["8.8.8.8", "2606:4700:4700::1111"] {
            assert!(allowed_ip(ip.parse().unwrap(), false, false), "{ip}");
        }
        // Loopback stays unreachable unless an isolated test policy opts in.
        for ip in ["127.0.0.1", "127.0.0.2", "::1", "::ffff:127.0.0.1"] {
            assert!(!allowed_ip(ip.parse().unwrap(), false, false), "{ip}");
            assert!(!allowed_ip(ip.parse().unwrap(), true, false), "{ip}");
            assert!(allowed_ip(ip.parse().unwrap(), false, true), "{ip}");
        }
        for ip in ["0.0.0.0", "169.254.169.254", "::", "fe80::1"] {
            assert!(!allowed_ip(ip.parse().unwrap(), false, true), "{ip}");
        }
    }
}
