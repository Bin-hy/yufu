//! Signed CSRF evidence is scoped to the issuer and a distinct browser context.
use super::{
    budget::{Clock, healthy_reading},
    error::SecurityError,
    identity::{PrincipalKind, PrincipalRef, SecretToken},
};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, decode_header, encode,
};
use sha2::{Digest, Sha256};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CsrfClaims {
    iss: String,
    aud: String,
    iat: u64,
    exp: u64,
    nonce: String,
    binding: String,
}

pub struct CsrfProtector(String);
#[derive(Clone, Copy)]
pub enum CsrfBinding<'a> {
    LegacyRefresh(Option<&'a str>),
    CenterRootDigest(&'a [u8; 32]),
    AnonymousBrowserDigest(&'a [u8; 32]),
}
impl CsrfProtector {
    pub fn new(key: String) -> Result<Self, SecurityError> {
        if key.is_empty() {
            return Err(SecurityError::DependencyUnavailable);
        }
        Ok(Self(key))
    }
    pub fn issue(
        &self,
        issuer: &str,
        refresh: Option<&str>,
        clock: &dyn Clock,
    ) -> Result<SecretToken, SecurityError> {
        self.issue_bound(issuer, CsrfBinding::LegacyRefresh(refresh), clock)
    }
    pub fn issue_bound(
        &self,
        issuer: &str,
        context: CsrfBinding<'_>,
        clock: &dyn Clock,
    ) -> Result<SecretToken, SecurityError> {
        PrincipalRef::new(issuer, PrincipalKind::Machine, "csrf")?;
        let iat = healthy_reading(clock)?.unix_millis / 1000;
        let claims = CsrfClaims {
            iss: issuer.into(),
            aud: "cobotusr-csrf".into(),
            iat,
            exp: iat
                .checked_add(900)
                .ok_or(SecurityError::DependencyUnavailable)?,
            nonce: uuid::Uuid::new_v4().to_string(),
            binding: binding(context),
        };
        let mut header = Header::new(Algorithm::HS256);
        header.typ = Some("csrf+jwt".into());
        SecretToken::new(
            encode(
                &header,
                &claims,
                &EncodingKey::from_secret(self.0.as_bytes()),
            )
            .map_err(|_| SecurityError::DependencyUnavailable)?,
        )
    }
    pub fn verify(
        &self,
        token: &SecretToken,
        issuer: &str,
        refresh: Option<&str>,
        clock: &dyn Clock,
    ) -> Result<(), SecurityError> {
        self.verify_bound(token, issuer, CsrfBinding::LegacyRefresh(refresh), clock)
    }
    pub fn verify_bound(
        &self,
        token: &SecretToken,
        issuer: &str,
        context: CsrfBinding<'_>,
        clock: &dyn Clock,
    ) -> Result<(), SecurityError> {
        let now = healthy_reading(clock)?.unix_millis;
        if token.expose_secret().len() > 4096 {
            return Err(SecurityError::Forbidden);
        }
        let header = decode_header(token.expose_secret()).map_err(|_| SecurityError::Forbidden)?;
        if header.alg != Algorithm::HS256 || header.typ.as_deref() != Some("csrf+jwt") {
            return Err(SecurityError::Forbidden);
        }
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[issuer]);
        validation.set_audience(&["cobotusr-csrf"]);
        validation.validate_exp = false;
        validation.validate_nbf = false;
        let claims = decode::<CsrfClaims>(
            token.expose_secret(),
            &DecodingKey::from_secret(self.0.as_bytes()),
            &validation,
        )
        .map_err(|_| SecurityError::Forbidden)?
        .claims;
        if claims.binding != binding(context)
            || claims.exp <= claims.iat
            || claims.exp.saturating_mul(1000) <= now.saturating_add(1000)
            || claims.iat.saturating_mul(1000) > now.saturating_add(1000)
            || uuid::Uuid::parse_str(&claims.nonce).is_err()
        {
            return Err(SecurityError::Forbidden);
        }
        Ok(())
    }
}

fn binding(context: CsrfBinding<'_>) -> String {
    let mut hash = Sha256::new();
    match context {
        CsrfBinding::LegacyRefresh(refresh) => {
            hash.update(b"cobotusr-csrf-v1\0");
            hash.update(if refresh.is_some() {
                b"session\0".as_slice()
            } else {
                b"bootstrap\0".as_slice()
            });
            if let Some(refresh) = refresh {
                hash.update(refresh.as_bytes());
            }
        }
        CsrfBinding::CenterRootDigest(digest) => {
            hash.update(b"cobotusr-csrf-v2\0center\0");
            hash.update(digest);
        }
        CsrfBinding::AnonymousBrowserDigest(digest) => {
            hash.update(b"cobotusr-csrf-v2\0anonymous\0");
            hash.update(digest);
        }
    }
    format!("{:x}", hash.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::budget::tests::ManualClock;
    #[test]
    fn identity_csrf_contexts_cannot_be_exchanged_and_reads_reuse_evidence() {
        let clock = ManualClock::new();
        let signer = CsrfProtector::new("context-key".into()).unwrap();
        let digest = [3; 32];
        let other = [4; 32];
        let contexts = [
            CsrfBinding::LegacyRefresh(None),
            CsrfBinding::LegacyRefresh(Some("legacy")),
            CsrfBinding::CenterRootDigest(&digest),
            CsrfBinding::AnonymousBrowserDigest(&digest),
        ];
        for (index, context) in contexts.iter().enumerate() {
            let token = signer
                .issue_bound("https://id.example", *context, &clock)
                .unwrap();
            for (candidate, binding) in contexts.iter().enumerate() {
                assert_eq!(
                    signer
                        .verify_bound(&token, "https://id.example", *binding, &clock)
                        .is_ok(),
                    index == candidate
                );
            }
            for _ in 0..4 {
                signer
                    .verify_bound(&token, "https://id.example", *context, &clock)
                    .unwrap();
            }
        }
        let token = signer
            .issue_bound(
                "https://id.example",
                CsrfBinding::CenterRootDigest(&digest),
                &clock,
            )
            .unwrap();
        assert_eq!(
            signer.verify_bound(
                &token,
                "https://id.example",
                CsrfBinding::CenterRootDigest(&other),
                &clock
            ),
            Err(SecurityError::Forbidden)
        );
    }
    #[test]
    fn security_t25_csrf_is_signed_and_bound_to_issuer_and_refresh() {
        let clock = ManualClock::new();
        let signer = CsrfProtector::new("csrf-test-secret".into()).unwrap();
        let token = signer
            .issue("https://idp.test", Some("first-refresh"), &clock)
            .unwrap();
        signer
            .verify(&token, "https://idp.test", Some("first-refresh"), &clock)
            .unwrap();
        for (issuer, refresh) in [
            ("https://other.test", Some("first-refresh")),
            ("https://idp.test", Some("second-refresh")),
            ("https://idp.test", None),
        ] {
            assert_eq!(
                signer.verify(&token, issuer, refresh, &clock),
                Err(SecurityError::Forbidden)
            );
        }
        let other = CsrfProtector::new("another-secret".into()).unwrap();
        assert_eq!(
            other.verify(&token, "https://idp.test", Some("first-refresh"), &clock),
            Err(SecurityError::Forbidden)
        );
    }
    #[test]
    fn security_t25_csrf_challenge_expires_and_unhealthy_clock_fails_closed() {
        let clock = ManualClock::new();
        let signer = CsrfProtector::new("csrf-test-secret".into()).unwrap();
        let token = signer.issue("https://idp.test", None, &clock).unwrap();
        signer
            .verify(&token, "https://idp.test", None, &clock)
            .unwrap();
        clock.advance(900_000);
        assert_eq!(
            signer.verify(&token, "https://idp.test", None, &clock),
            Err(SecurityError::Forbidden)
        );
        clock.0.lock().unwrap().synchronized = false;
        assert_eq!(
            signer.issue("https://idp.test", None, &clock).err(),
            Some(SecurityError::DependencyUnavailable)
        );
        assert_eq!(
            signer.verify(&token, "https://idp.test", None, &clock),
            Err(SecurityError::DependencyUnavailable)
        );
    }
    #[test]
    fn security_t25_csrf_tampering_and_cross_profile_are_rejected() {
        let clock = ManualClock::new();
        let signer = CsrfProtector::new("csrf-test-secret".into()).unwrap();
        let token = signer.issue("https://idp.test", None, &clock).unwrap();
        let mut bytes = token.expose_secret().as_bytes().to_vec();
        let middle = bytes.len() / 2;
        bytes[middle] = if bytes[middle] == b'A' { b'B' } else { b'A' };
        assert_eq!(
            signer.verify(
                &SecretToken::new(String::from_utf8(bytes).unwrap()).unwrap(),
                "https://idp.test",
                None,
                &clock
            ),
            Err(SecurityError::Forbidden)
        );
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
        header.typ = Some("at+jwt".into());
        let wrong=jsonwebtoken::encode(&header,&serde_json::json!({"iss":"https://idp.test","aud":"cobotusr-csrf","exp":1000,"nonce":"known","binding":""}),&jsonwebtoken::EncodingKey::from_secret(b"csrf-test-secret")).unwrap();
        assert_eq!(
            signer.verify(
                &SecretToken::new(wrong).unwrap(),
                "https://idp.test",
                None,
                &clock
            ),
            Err(SecurityError::Forbidden)
        );
    }
}
