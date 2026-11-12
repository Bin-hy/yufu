use super::{account::SecretDigest, error::IdentityError};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

/// This provider keeps the original grant's scopes throughout its family.
/// A different scope set requires a new authorization and consent decision.
pub fn validate_refresh_scope(
    original: &super::application::IdentityScopeSet,
    requested: Option<&str>,
) -> Result<(), IdentityError> {
    if let Some(requested) = requested {
        let requested = super::application::IdentityScopeSet::parse(requested)
            .map_err(|_| IdentityError::InvalidScope)?;
        if &requested != original {
            return Err(IdentityError::InvalidScope);
        }
    }
    Ok(())
}

pub fn verify_s256(challenge: &str, verifier: &str) -> Result<(), IdentityError> {
    if !(43..=128).contains(&verifier.len())
        || !verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'))
    {
        return Err(IdentityError::AccessDenied);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(challenge)
        .map_err(|_| IdentityError::AccessDenied)?;
    let digest: [u8; 32] = bytes.try_into().map_err(|_| IdentityError::AccessDenied)?;
    if URL_SAFE_NO_PAD.encode(digest) != challenge
        || !SecretDigest::from_bytes(digest).matches(verifier)
    {
        return Err(IdentityError::AccessDenied);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_token_refresh_scope_retains_the_original_grant() {
        let scopes = super::super::application::IdentityScopeSet::parse("openid profile").unwrap();
        assert_eq!(validate_refresh_scope(&scopes, None), Ok(()));
        assert_eq!(
            validate_refresh_scope(&scopes, Some("profile openid")),
            Ok(())
        );
        for value in [
            "",
            "openid",
            "openid profile email",
            "openid offline_access",
        ] {
            assert_eq!(
                validate_refresh_scope(&scopes, Some(value)),
                Err(IdentityError::InvalidScope)
            );
        }
    }
    #[test]
    fn identity_token_s256_rfc_example_and_verifier_boundaries() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert_eq!(verify_s256(challenge, verifier), Ok(()));
        for value in [
            "",
            "short",
            &"a".repeat(42),
            &"a".repeat(129),
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjX%",
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXz",
        ] {
            assert_eq!(
                verify_s256(challenge, value),
                Err(IdentityError::AccessDenied)
            );
        }
        for value in [
            "",
            &format!("{challenge}="),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB",
        ] {
            assert!(verify_s256(value, verifier).is_err());
        }
    }
}
