//! Provider secrets are encrypted with a deployment-held master key.
//!
//! The key lives only in `YUFU_EXTERNAL_SECRET_KEY`; ciphertext in PostgreSQL is
//! useless without it. A missing key fails closed whenever configured secrets
//! exist, and plaintext never enters Debug output, logs or API responses.
use crate::domain::identity::error::IdentityError;
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use auth::security::identity::SecretToken;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{RngCore, rngs::OsRng};

const KEY_ENV: &str = "YUFU_EXTERNAL_SECRET_KEY";
const VERSION_ENV: &str = "YUFU_EXTERNAL_SECRET_KEY_VERSION";
const NONCE_BYTES: usize = 12;

#[derive(Clone)]
pub struct SecretCipher {
    key: [u8; 32],
    key_version: u32,
}

impl SecretCipher {
    pub fn from_env() -> Result<Option<Self>, IdentityError> {
        let Ok(raw) = std::env::var(KEY_ENV) else {
            return Ok(None);
        };
        if raw.trim().is_empty() {
            return Ok(None);
        }
        let key = decode_key(raw.trim())?;
        let key_version = match std::env::var(VERSION_ENV) {
            Ok(value) => value
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|version| *version > 0)
                .ok_or(IdentityError::DependencyUnavailable)?,
            Err(_) => 1,
        };
        Ok(Some(Self { key, key_version }))
    }

    pub fn from_key(key: [u8; 32], key_version: u32) -> Result<Self, IdentityError> {
        if key_version == 0 {
            return Err(IdentityError::InvalidInput);
        }
        Ok(Self { key, key_version })
    }

    pub fn key_version(&self) -> u32 {
        self.key_version
    }

    pub fn encrypt(&self, provider_key: &str, plaintext: &str) -> Result<String, IdentityError> {
        if provider_key.is_empty() || plaintext.is_empty() {
            return Err(IdentityError::InvalidInput);
        }
        let cipher = self.cipher()?;
        let mut nonce = [0u8; NONCE_BYTES];
        OsRng.fill_bytes(&mut nonce);
        let aad = self.aad(provider_key);
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext.as_bytes(),
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        Ok(format!(
            "v1.{}.{}.{}",
            self.key_version,
            URL_SAFE_NO_PAD.encode(nonce),
            URL_SAFE_NO_PAD.encode(ciphertext)
        ))
    }

    pub fn decrypt(&self, provider_key: &str, encoded: &str) -> Result<SecretToken, IdentityError> {
        if provider_key.is_empty() || encoded.is_empty() {
            return Err(IdentityError::InvalidInput);
        }
        let parts = encoded.split('.').collect::<Vec<_>>();
        if parts.len() != 4 || parts[0] != "v1" {
            return Err(IdentityError::DependencyUnavailable);
        }
        let version = parts[1]
            .parse::<u32>()
            .ok()
            .filter(|version| *version > 0)
            .ok_or(IdentityError::DependencyUnavailable)?;
        if version != self.key_version {
            return Err(IdentityError::DependencyUnavailable);
        }
        let nonce = URL_SAFE_NO_PAD
            .decode(parts[2])
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        if nonce.len() != NONCE_BYTES {
            return Err(IdentityError::DependencyUnavailable);
        }
        let ciphertext = URL_SAFE_NO_PAD
            .decode(parts[3])
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        let aad = self.aad(provider_key);
        let plaintext = self
            .cipher()?
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: ciphertext.as_slice(),
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        let value =
            String::from_utf8(plaintext).map_err(|_| IdentityError::DependencyUnavailable)?;
        SecretToken::new(value).map_err(|_| IdentityError::DependencyUnavailable)
    }

    fn cipher(&self) -> Result<Aes256Gcm, IdentityError> {
        Aes256Gcm::new_from_slice(&self.key).map_err(|_| IdentityError::DependencyUnavailable)
    }

    fn aad(&self, provider_key: &str) -> String {
        format!("{provider_key}|{}", self.key_version)
    }
}

impl std::fmt::Debug for SecretCipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretCipher(REDACTED)")
    }
}

fn decode_key(raw: &str) -> Result<[u8; 32], IdentityError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(raw)
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(raw))
        .map_err(|_| IdentityError::DependencyUnavailable)?;
    bytes
        .try_into()
        .map_err(|_| IdentityError::DependencyUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cipher() -> SecretCipher {
        SecretCipher::from_key([7u8; 32], 1).unwrap()
    }

    #[test]
    fn secret_cipher_round_trips_and_binds_provider() {
        let cipher = cipher();
        let encoded = cipher.encrypt("github", "github-secret-value").unwrap();
        assert!(encoded.starts_with("v1.1."));
        assert!(!encoded.contains("github-secret-value"));
        assert_eq!(
            cipher.decrypt("github", &encoded).unwrap().expose_secret(),
            "github-secret-value"
        );
        assert!(cipher.decrypt("qq", &encoded).is_err());
        assert!(cipher.decrypt("github", "v1.1.bad.bad").is_err());
        assert!(cipher.decrypt("github", "v2.1.a.b").is_err());
        assert!(!format!("{cipher:?}").contains("7"));
    }

    #[test]
    fn secret_cipher_rejects_tampering_and_wrong_keys() {
        let cipher = cipher();
        let mut encoded = cipher.encrypt("qq", "qq-secret").unwrap();
        let last = encoded.pop().unwrap();
        encoded.push(if last == 'A' { 'B' } else { 'A' });
        assert!(cipher.decrypt("qq", &encoded).is_err());

        let other = SecretCipher::from_key([9u8; 32], 1).unwrap();
        let encoded = cipher.encrypt("qq", "qq-secret").unwrap();
        assert!(other.decrypt("qq", &encoded).is_err());
    }

    #[test]
    fn secret_cipher_validates_inputs_and_key_version() {
        let cipher = cipher();
        assert!(cipher.encrypt("", "x").is_err());
        assert!(cipher.encrypt("github", "").is_err());
        assert!(SecretCipher::from_key([0; 32], 0).is_err());
        let version_two = SecretCipher::from_key([7u8; 32], 2).unwrap();
        let encoded = version_two.encrypt("github", "value").unwrap();
        assert!(cipher.decrypt("github", &encoded).is_err());
        assert_eq!(
            version_two
                .decrypt("github", &encoded)
                .unwrap()
                .expose_secret(),
            "value"
        );
    }
}
