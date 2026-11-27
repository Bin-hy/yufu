//! Runtime catalog of enabled providers. Disabled rows never reach a login page.
use super::{
    adapter::{ExternalIdentityAdapter, ExternalProviderConfig},
    secret::SecretCipher,
};
use crate::domain::identity::{error::IdentityError, external::ExternalProviderKey};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct PublicProvider {
    pub key: ExternalProviderKey,
    pub family: &'static str,
    pub label: &'static str,
    pub mode: &'static str,
    pub enabled: bool,
}

/// One row read from `identity_external_provider`, kept free of database types.
#[derive(Clone)]
pub struct StoredProvider {
    pub provider_key: String,
    pub family: String,
    pub enabled: bool,
    pub client_id: String,
    pub secret_ciphertext: Option<String>,
    pub config: serde_json::Value,
}

#[derive(Default)]
pub struct ExternalIdentityRegistry {
    adapters: BTreeMap<ExternalProviderKey, Arc<dyn ExternalIdentityAdapter>>,
    configs: BTreeMap<ExternalProviderKey, ExternalProviderConfig>,
    catalog: Vec<PublicProvider>,
}

impl ExternalIdentityRegistry {
    pub async fn load(
        db: &DatabaseConnection,
        cipher: Option<&SecretCipher>,
        adapters: Vec<Arc<dyn ExternalIdentityAdapter>>,
    ) -> Result<Self, IdentityError> {
        let rows = crate::entity::identity_external_provider::Entity::find()
            .all(db)
            .await
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        let stored = rows
            .into_iter()
            .map(|row| StoredProvider {
                provider_key: row.provider_key,
                family: row.family,
                enabled: row.enabled,
                client_id: row.client_id,
                secret_ciphertext: row.secret_ciphertext,
                config: row.config,
            })
            .collect();
        Self::build(stored, cipher, adapters)
    }

    pub fn build(
        rows: Vec<StoredProvider>,
        cipher: Option<&SecretCipher>,
        adapters: Vec<Arc<dyn ExternalIdentityAdapter>>,
    ) -> Result<Self, IdentityError> {
        let adapters = adapters
            .into_iter()
            .map(|adapter| (adapter.key(), adapter))
            .collect::<BTreeMap<_, _>>();
        let mut configs = BTreeMap::new();
        let mut catalog = Vec::new();
        for row in rows {
            let key = row.provider_key.parse::<ExternalProviderKey>()?;
            if row.family != key.family() {
                return Err(IdentityError::DependencyUnavailable);
            }
            if !row.enabled {
                continue;
            }
            if row.client_id.is_empty() {
                return Err(IdentityError::DependencyUnavailable);
            }
            let ciphertext = row
                .secret_ciphertext
                .as_deref()
                .ok_or(IdentityError::DependencyUnavailable)?;
            let secret = cipher
                .ok_or(IdentityError::DependencyUnavailable)?
                .decrypt(key.as_str(), ciphertext)?;
            if !adapters.contains_key(&key) {
                return Err(IdentityError::DependencyUnavailable);
            }
            configs.insert(
                key,
                ExternalProviderConfig {
                    key,
                    client_id: row.client_id,
                    secret: Arc::new(secret),
                    config: row.config,
                },
            );
            catalog.push(PublicProvider {
                key,
                family: key.family(),
                label: key.label(),
                mode: if key.is_native() { "native" } else { "oauth" },
                enabled: true,
            });
        }
        catalog.sort_by_key(|provider| provider.key);
        Ok(Self {
            adapters,
            configs,
            catalog,
        })
    }

    pub fn public_catalog(&self) -> &[PublicProvider] {
        &self.catalog
    }

    pub fn adapter(
        &self,
        key: ExternalProviderKey,
    ) -> Result<&Arc<dyn ExternalIdentityAdapter>, IdentityError> {
        self.adapters.get(&key).ok_or(IdentityError::NotFound)
    }

    pub fn config(
        &self,
        key: ExternalProviderKey,
    ) -> Result<&ExternalProviderConfig, IdentityError> {
        self.configs.get(&key).ok_or(IdentityError::NotFound)
    }

    pub fn is_enabled(&self, key: ExternalProviderKey) -> bool {
        self.configs.contains_key(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::identity::external::ExternalIdentity;
    use crate::service::identity::external::adapter::{
        AdapterCapabilities, BeginContext, BeginOutcome, CompleteContext,
    };

    struct FakeAdapter(ExternalProviderKey);

    #[async_trait::async_trait]
    impl ExternalIdentityAdapter for FakeAdapter {
        fn key(&self) -> ExternalProviderKey {
            self.0
        }
        fn capabilities(&self) -> AdapterCapabilities {
            AdapterCapabilities {
                pkce: true,
                nonce: false,
                native: false,
                union_id: false,
            }
        }
        async fn begin(&self, _: BeginContext<'_>) -> Result<BeginOutcome, IdentityError> {
            Err(IdentityError::DependencyUnavailable)
        }
        async fn complete(
            &self,
            _: CompleteContext<'_>,
        ) -> Result<ExternalIdentity, IdentityError> {
            Err(IdentityError::DependencyUnavailable)
        }
    }

    fn row(key: &str, enabled: bool, secret: Option<String>) -> StoredProvider {
        let parsed = key.parse::<ExternalProviderKey>().unwrap();
        StoredProvider {
            provider_key: key.into(),
            family: parsed.family().into(),
            enabled,
            client_id: if enabled {
                "client-id".into()
            } else {
                String::new()
            },
            secret_ciphertext: secret,
            config: serde_json::json!({}),
        }
    }

    #[test]
    fn registry_hides_disabled_and_requires_complete_enabled_rows() {
        let cipher = SecretCipher::from_key([3u8; 32], 1).unwrap();
        let adapters =
            vec![Arc::new(FakeAdapter(ExternalProviderKey::GitHub))
                as Arc<dyn ExternalIdentityAdapter>];
        let registry = ExternalIdentityRegistry::build(
            vec![row("github", false, None), row("qq", false, None)],
            Some(&cipher),
            adapters.clone(),
        )
        .unwrap();
        assert!(registry.public_catalog().is_empty());
        assert!(!registry.is_enabled(ExternalProviderKey::GitHub));

        let secret = cipher.encrypt("github", "secret").unwrap();
        let registry = ExternalIdentityRegistry::build(
            vec![row("github", true, Some(secret))],
            Some(&cipher),
            adapters,
        )
        .unwrap();
        assert_eq!(registry.public_catalog().len(), 1);
        assert_eq!(
            registry.public_catalog()[0].key,
            ExternalProviderKey::GitHub
        );
        assert!(registry.config(ExternalProviderKey::GitHub).is_ok());
    }

    #[test]
    fn registry_fails_closed_on_missing_key_adapter_or_ciphertext() {
        let cipher = SecretCipher::from_key([3u8; 32], 1).unwrap();
        let adapters =
            vec![Arc::new(FakeAdapter(ExternalProviderKey::GitHub))
                as Arc<dyn ExternalIdentityAdapter>];
        let secret = cipher.encrypt("github", "secret").unwrap();
        assert!(
            ExternalIdentityRegistry::build(
                vec![row("github", true, Some(secret.clone()))],
                None,
                adapters.clone(),
            )
            .is_err()
        );
        assert!(
            ExternalIdentityRegistry::build(
                vec![row("github", true, None)],
                Some(&cipher),
                adapters.clone(),
            )
            .is_err()
        );
        assert!(
            ExternalIdentityRegistry::build(
                vec![row("github", true, Some(secret))],
                Some(&cipher),
                Vec::new(),
            )
            .is_err()
        );
        let wrong = SecretCipher::from_key([4u8; 32], 1).unwrap();
        let secret = cipher.encrypt("github", "secret").unwrap();
        assert!(
            ExternalIdentityRegistry::build(
                vec![row("github", true, Some(secret))],
                Some(&wrong),
                adapters,
            )
            .is_err()
        );
    }
}
