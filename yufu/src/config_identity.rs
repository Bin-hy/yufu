//! Fixed deployment identity and bounded browser/protocol lifetimes.
use crate::config::{Config, is_loopback_host};
use auth::token_manager::IdTokenSigner;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Identity {
    pub background_workers_enabled: bool,
    pub sso_session_ttl_seconds: u64,
    pub flow_ttl_seconds: u64,
    pub max_pending_flows: usize,
    pub code_ttl_seconds: u64,
    pub access_token_ttl_seconds: u64,
    pub invitation_ttl_seconds: u64,
    pub password_reset_ttl_seconds: u64,
    pub logout_timeout_millis: u64,
    pub logout_retry_seconds: Vec<u64>,
    pub logout_max_age_seconds: u64,
    /// Exact HTTPS origins allowed for enterprise private network deliveries.
    pub logout_private_origins: Vec<String>,
    /// Users one legacy full-scope write may touch. The security budget still
    /// bounds the work; this only decides when the write is refused up front.
    pub legacy_governance_max_users: usize,
    pub audit_retention_seconds: u64,
    pub operation_retention_seconds: u64,
}

impl Default for Identity {
    fn default() -> Self {
        Self {
            background_workers_enabled: true,
            sso_session_ttl_seconds: 43_200,
            flow_ttl_seconds: 600,
            max_pending_flows: 8,
            code_ttl_seconds: 60,
            access_token_ttl_seconds: 300,
            invitation_ttl_seconds: 86_400,
            password_reset_ttl_seconds: 1_800,
            logout_timeout_millis: 2_000,
            logout_retry_seconds: vec![5, 30, 120, 600, 3_600],
            logout_max_age_seconds: 86_400,
            logout_private_origins: Vec::new(),
            legacy_governance_max_users: 1_000,
            audit_retention_seconds: 90 * 86_400,
            operation_retention_seconds: 86_400,
        }
    }
}

impl Config {
    pub fn issuer(&self) -> String {
        if !self.oidc.issuer.is_empty() {
            self.oidc.issuer.clone()
        } else if !self.http.public.is_empty() {
            self.http.public.clone()
        } else {
            format!("http://{}", self.http.listen)
        }
    }

    pub fn validate_identity_policy(&self) -> anyhow::Result<()> {
        self.validate_browser_cookie_policy()?;
        let issuer = self.issuer();
        let url =
            url::Url::parse(&issuer).map_err(|_| anyhow::anyhow!("invalid identity issuer"))?;
        anyhow::ensure!(
            url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "invalid identity issuer"
        );
        let development = self.auth.allow_insecure_loopback_cookies;
        if !development {
            anyhow::ensure!(
                !self.oidc.issuer.is_empty() || !self.http.public.is_empty(),
                "production requires an explicit fixed issuer"
            );
            anyhow::ensure!(url.scheme() == "https", "production issuer requires HTTPS");
            anyhow::ensure!(
                self.oidc.signing_algorithm.eq_ignore_ascii_case("RS256"),
                "production identity signing requires RS256"
            );
        } else {
            anyhow::ensure!(
                url.host_str().is_some_and(is_loopback_host),
                "development issuer requires loopback"
            );
            anyhow::ensure!(
                self.oidc.signing_algorithm.eq_ignore_ascii_case("RS256")
                    || self.oidc.signing_algorithm.eq_ignore_ascii_case("HS256"),
                "unsupported development signing algorithm"
            );
        }
        let policy = &self.identity;
        anyhow::ensure!(
            (1..=43_200).contains(&policy.sso_session_ttl_seconds),
            "invalid SSO lifetime"
        );
        anyhow::ensure!(
            (1..=600).contains(&policy.flow_ttl_seconds),
            "invalid flow lifetime"
        );
        anyhow::ensure!(
            (1..=8).contains(&policy.max_pending_flows),
            "invalid pending flow limit"
        );
        anyhow::ensure!(
            (1..=60).contains(&policy.code_ttl_seconds),
            "invalid authorization code lifetime"
        );
        anyhow::ensure!(
            (1..=policy.sso_session_ttl_seconds).contains(&policy.access_token_ttl_seconds),
            "invalid identity access lifetime"
        );
        anyhow::ensure!(
            (1..=7 * 86_400).contains(&policy.invitation_ttl_seconds),
            "invalid invitation lifetime"
        );
        anyhow::ensure!(
            (1..=1_800).contains(&policy.password_reset_ttl_seconds),
            "invalid reset lifetime"
        );
        anyhow::ensure!(
            (1..=2_000).contains(&policy.logout_timeout_millis),
            "invalid logout timeout"
        );
        anyhow::ensure!(
            (1..=86_400).contains(&policy.logout_max_age_seconds),
            "invalid logout retry horizon"
        );
        anyhow::ensure!(
            (1..=10_000).contains(&policy.legacy_governance_max_users),
            "invalid legacy governance bound"
        );
        anyhow::ensure!(
            !policy.logout_retry_seconds.is_empty()
                && policy.logout_retry_seconds.len() <= 16
                && policy
                    .logout_retry_seconds
                    .iter()
                    .all(|delay| *delay > 0 && *delay <= policy.logout_max_age_seconds)
                && policy
                    .logout_retry_seconds
                    .windows(2)
                    .all(|pair| pair[0] <= pair[1]),
            "invalid logout retry schedule"
        );
        anyhow::ensure!(
            (86_400..=3650 * 86_400).contains(&policy.audit_retention_seconds),
            "invalid audit retention"
        );
        anyhow::ensure!(
            (600..=365 * 86_400).contains(&policy.operation_retention_seconds),
            "invalid operation retention"
        );
        for allowed in &policy.logout_private_origins {
            let origin = url::Url::parse(allowed)
                .map_err(|_| anyhow::anyhow!("invalid logout private origin"))?;
            anyhow::ensure!(
                origin.scheme() == "https"
                    && origin.host_str().is_some()
                    && origin.username().is_empty()
                    && origin.password().is_none()
                    && origin.query().is_none()
                    && origin.fragment().is_none()
                    && origin.origin().ascii_serialization() == *allowed,
                "invalid logout private origin"
            );
        }
        Ok(())
    }

    /// Existing keys are read without modification. Generation is allowed only
    /// in explicit loopback development, with exclusive creation and mode 0600.
    pub async fn load_identity_signer(&self) -> anyhow::Result<IdTokenSigner> {
        self.validate_identity_policy()?;
        let expires = self
            .oidc
            .access_token_expire_seconds
            .unwrap_or(self.auth.exp_seconds);
        if self.oidc.signing_algorithm.eq_ignore_ascii_case("HS256") {
            return Ok(IdTokenSigner::from_hmac(&self.auth.secret, expires));
        }
        let path = if self.oidc.rsa_private_key_path.is_empty() {
            &self.auth.rsa_private_key_path
        } else {
            &self.oidc.rsa_private_key_path
        };
        anyhow::ensure!(!path.is_empty(), "persistent RSA key is required");
        let pem = match tokio::fs::read(path).await {
            Ok(pem) => pem,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && self.auth.allow_insecure_loopback_cookies =>
            {
                let pem = tokio::task::spawn_blocking(|| {
                    use rsa::pkcs8::EncodePrivateKey;
                    let key = rsa::PrivateKey::new(&mut getrandom::SysRng, 2048)
                        .map_err(|_| anyhow::anyhow!("RSA generation failed"))?;
                    key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
                        .map(|pem| pem.as_bytes().to_vec())
                        .map_err(|_| anyhow::anyhow!("RSA encoding failed"))
                })
                .await
                .map_err(|_| anyhow::anyhow!("RSA generation unavailable"))??;
                let parent = std::path::Path::new(path)
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty());
                if let Some(parent) = parent {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|_| anyhow::anyhow!("RSA storage unavailable"))?;
                }
                let mut options = tokio::fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                options.mode(0o600);
                match options.open(path).await {
                    Ok(mut file) => {
                        use tokio::io::AsyncWriteExt;
                        file.write_all(&pem)
                            .await
                            .map_err(|_| anyhow::anyhow!("RSA persistence failed"))?;
                        file.sync_all()
                            .await
                            .map_err(|_| anyhow::anyhow!("RSA persistence failed"))?;
                        pem
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        tokio::fs::read(path)
                            .await
                            .map_err(|_| anyhow::anyhow!("RSA storage unavailable"))?
                    }
                    Err(_) => return Err(anyhow::anyhow!("RSA storage unavailable")),
                }
            }
            Err(_) => return Err(anyhow::anyhow!("persistent RSA key unavailable")),
        };
        IdTokenSigner::from_rsa_pem(&pem, expires)
            .map_err(|_| anyhow::anyhow!("persistent RSA key invalid"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn production() -> Config {
        let mut config = Config::default();
        config.oidc.issuer = "https://identity.example".into();
        config
    }

    #[test]
    fn identity_config_requires_explicit_https_issuer_and_rs256() {
        assert!(Config::default().validate_identity_policy().is_err());
        assert!(production().validate_identity_policy().is_ok());
        for issuer in [
            "http://identity.example",
            "https://user:password@identity.example",
            "https://identity.example?secret=x",
            "https://identity.example#fragment",
            "bad",
        ] {
            let mut config = production();
            config.oidc.issuer = issuer.into();
            assert!(config.validate_identity_policy().is_err());
        }
        let mut config = production();
        config.oidc.signing_algorithm = "HS256".into();
        assert!(config.validate_identity_policy().is_err());
        config.auth.allow_insecure_loopback_cookies = true;
        config.http.listen = "127.0.0.1:4000".parse().unwrap();
        config.oidc.issuer = "http://localhost:4000".into();
        assert!(config.validate_identity_policy().is_ok());
        config.http.listen = "0.0.0.0:4000".parse().unwrap();
        assert!(config.validate_identity_policy().is_err());
    }

    #[test]
    fn identity_config_defaults_and_untrusted_limits_are_bounded() {
        let config = production();
        assert_eq!(config.identity.sso_session_ttl_seconds, 43_200);
        assert_eq!(config.identity.access_token_ttl_seconds, 300);
        assert_eq!(config.identity.max_pending_flows, 8);
        let changes: Vec<fn(&mut Identity)> = vec![
            |p| p.sso_session_ttl_seconds = 43_201,
            |p| p.flow_ttl_seconds = 601,
            |p| p.max_pending_flows = 9,
            |p| p.code_ttl_seconds = 61,
            |p| p.access_token_ttl_seconds = 43_201,
            |p| p.invitation_ttl_seconds = 0,
            |p| p.password_reset_ttl_seconds = 1_801,
            |p| p.logout_timeout_millis = 2_001,
            |p| p.logout_retry_seconds = vec![30, 5],
            |p| p.logout_max_age_seconds = 86_401,
            |p| p.legacy_governance_max_users = 0,
            |p| p.legacy_governance_max_users = 10_001,
            |p| p.audit_retention_seconds = 0,
            |p| p.operation_retention_seconds = 0,
            |p| p.logout_private_origins = vec!["https://internal.example/path".into()],
        ];
        for change in changes {
            let mut config = config.clone();
            change(&mut config.identity);
            assert!(config.validate_identity_policy().is_err());
        }
    }
}
