//! Fixed-endpoint outbound client. No redirects, no proxy, bounded body and time.
use crate::domain::identity::error::IdentityError;
use auth::security::identity::SecretToken;
use serde::de::DeserializeOwned;
use std::time::Duration;

const MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct ExternalHttpClient {
    client: reqwest::Client,
    timeout: Duration,
    allow_loopback_http: bool,
}

impl ExternalHttpClient {
    pub fn new(timeout: Duration, allow_loopback_http: bool) -> Result<Self, IdentityError> {
        if timeout.is_zero() || timeout > Duration::from_secs(15) {
            return Err(IdentityError::InvalidInput);
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .https_only(!allow_loopback_http)
            .http1_only()
            .user_agent(concat!("yufu/", env!("CARGO_PKG_VERSION")))
            .timeout(timeout)
            .build()
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        Ok(Self {
            client,
            timeout,
            allow_loopback_http,
        })
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        bearer: Option<&SecretToken>,
    ) -> Result<T, IdentityError> {
        let body = self.get_bytes(url, bearer).await?;
        serde_json::from_slice(&body).map_err(|_| IdentityError::DependencyUnavailable)
    }

    pub async fn get_text(
        &self,
        url: &str,
        bearer: Option<&SecretToken>,
    ) -> Result<String, IdentityError> {
        let body = self.get_bytes(url, bearer).await?;
        String::from_utf8(body).map_err(|_| IdentityError::DependencyUnavailable)
    }

    pub async fn post_form<T: DeserializeOwned>(
        &self,
        url: &str,
        form: &[(&str, &str)],
        accept_json: bool,
    ) -> Result<T, IdentityError> {
        let url = self.checked_url(url)?;
        let mut request = self.client.post(url).form(form);
        if accept_json {
            request = request.header(reqwest::header::ACCEPT, "application/json");
        }
        let response = request.send().await.map_err(map_error)?;
        let body = read_body(response).await?;
        serde_json::from_slice(&body).map_err(|_| IdentityError::DependencyUnavailable)
    }

    async fn get_bytes(
        &self,
        url: &str,
        bearer: Option<&SecretToken>,
    ) -> Result<Vec<u8>, IdentityError> {
        let url = self.checked_url(url)?;
        let mut request = self
            .client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(token) = bearer {
            request = request.bearer_auth(token.expose_secret());
        }
        let response = request.send().await.map_err(map_error)?;
        read_body(response).await
    }

    fn checked_url(&self, raw: &str) -> Result<url::Url, IdentityError> {
        let url = url::Url::parse(raw).map_err(|_| IdentityError::InvalidInput)?;
        if url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || url.port_or_known_default() == Some(0)
        {
            return Err(IdentityError::InvalidInput);
        }
        match url.scheme() {
            "https" => Ok(url),
            "http"
                if self.allow_loopback_http && is_loopback_host(url.host_str().unwrap_or("")) =>
            {
                Ok(url)
            }
            _ => Err(IdentityError::InvalidInput),
        }
    }
}

impl std::fmt::Debug for ExternalHttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalHttpClient")
            .field("timeout", &self.timeout)
            .field("allow_loopback_http", &self.allow_loopback_http)
            .finish()
    }
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

fn map_error(error: reqwest::Error) -> IdentityError {
    if error.is_timeout() {
        return IdentityError::DeadlineExceeded;
    }
    let status = error.status();
    match status {
        Some(status) if status.as_u16() == 401 || status.as_u16() == 403 => {
            IdentityError::Unauthenticated
        }
        Some(status) if status.is_client_error() => IdentityError::Unauthenticated,
        _ => IdentityError::DependencyUnavailable,
    }
}

async fn read_body(response: reqwest::Response) -> Result<Vec<u8>, IdentityError> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_BODY_BYTES as u64)
    {
        return Err(IdentityError::CapacityExceeded);
    }
    let body = response
        .bytes()
        .await
        .map_err(|_| IdentityError::DependencyUnavailable)?;
    if body.len() > MAX_BODY_BYTES {
        return Err(IdentityError::CapacityExceeded);
    }
    if status.is_client_error() {
        tracing::warn!(status = %status, "external HTTP client error");
        return Err(IdentityError::Unauthenticated);
    }
    if !status.is_success() {
        tracing::warn!(status = %status, "external HTTP server error");
        return Err(IdentityError::DependencyUnavailable);
    }
    Ok(body.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outbound_client_rejects_untrusted_urls() {
        let client = ExternalHttpClient::new(Duration::from_secs(5), false).unwrap();
        for url in [
            "http://github.com/login/oauth/access_token",
            "file:///etc/passwd",
            "https://user:pass@example.test/x",
            "https://example.test/x#fragment",
            "not a url",
        ] {
            assert!(client.checked_url(url).is_err(), "{url}");
        }
        assert!(
            client
                .checked_url("https://github.com/login/oauth/access_token")
                .is_ok()
        );
        let fixture = ExternalHttpClient::new(Duration::from_secs(5), true).unwrap();
        assert!(fixture.checked_url("http://127.0.0.1:8080/x").is_ok());
        assert!(fixture.checked_url("http://10.0.0.1:8080/x").is_err());
        assert!(ExternalHttpClient::new(Duration::ZERO, false).is_err());
        assert!(ExternalHttpClient::new(Duration::from_secs(16), false).is_err());
    }
}
