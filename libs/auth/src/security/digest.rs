//! SHA-256 request fingerprints for authorization audit decisions.
use super::error::SecurityError;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RequestDigest([u8; 32]);
impl RequestDigest {
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }
}
impl TryFrom<String> for RequestDigest {
    type Error = SecurityError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() != 43 {
            return Err(SecurityError::InvalidCredential);
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(value)
            .map_err(|_| SecurityError::InvalidCredential)?;
        Ok(Self(
            bytes
                .try_into()
                .map_err(|_| SecurityError::InvalidCredential)?,
        ))
    }
}
impl From<RequestDigest> for String {
    fn from(value: RequestDigest) -> String {
        URL_SAFE_NO_PAD.encode(value.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn request_digest_is_deterministic_and_validates_encoding() {
        let digest = RequestDigest::of(b"authorization decision");
        assert_eq!(
            RequestDigest::try_from(String::from(digest)).unwrap(),
            digest
        );
        assert_ne!(digest, RequestDigest::of(b"other decision"));
        assert!(RequestDigest::try_from("invalid".to_owned()).is_err());
        assert!(RequestDigest::try_from("!".repeat(43)).is_err());
    }
}
