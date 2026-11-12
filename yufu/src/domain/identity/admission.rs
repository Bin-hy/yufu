use super::account::{AccountSnapshot, ensure_enabled};
use super::application::{AppEnvironmentKind, IdentityScopeSet};
use super::error::IdentityError;
use super::session::BrowserSessionSnapshot;
use auth::security::identity::PrincipalRef;
use auth::security::policy::CredentialBounds;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AdmissionSources {
    pub direct_assignment: bool,
    pub group_assignments: usize,
    pub qualified_application_member: bool,
    pub explicit_test_user: bool,
}

impl AdmissionSources {
    pub fn eligible(self, kind: AppEnvironmentKind) -> bool {
        match kind {
            AppEnvironmentKind::Production => self.direct_assignment || self.group_assignments > 0,
            AppEnvironmentKind::Development => {
                self.qualified_application_member || self.explicit_test_user
            }
        }
    }
}

pub struct AdmissionInput<'a> {
    pub account: &'a AccountSnapshot,
    pub root: &'a BrowserSessionSnapshot,
    pub now_millis: u64,
    pub environment_id: Uuid,
    pub environment_kind: AppEnvironmentKind,
    pub application_active: bool,
    pub owner_qualified: bool,
    pub client_enabled: bool,
    pub active_version_id: Option<Uuid>,
    pub lifecycle_epoch: u64,
    pub generation: AccessGeneration,
    pub sources: AdmissionSources,
    pub approved_scopes: &'a IdentityScopeSet,
    pub requested_scopes: &'a IdentityScopeSet,
}

/// Current admission is an authorization fact, never a deserializable request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedAdmission {
    pub(crate) principal: PrincipalRef,
    pub(crate) sso_session_id: Uuid,
    pub(crate) environment_id: Uuid,
    pub(crate) config_version_id: Uuid,
    pub(crate) auth_epoch: u64,
    pub(crate) lifecycle_epoch: u64,
    pub(crate) access_generation: u64,
    pub(crate) scopes: IdentityScopeSet,
    pub(crate) auth_time: u64,
}

impl ManagedAdmission {
    pub fn principal(&self) -> &PrincipalRef {
        &self.principal
    }
    pub fn sso_session_id(&self) -> Uuid {
        self.sso_session_id
    }
    pub fn environment_id(&self) -> Uuid {
        self.environment_id
    }
    pub fn config_version_id(&self) -> Uuid {
        self.config_version_id
    }
    pub fn auth_epoch(&self) -> u64 {
        self.auth_epoch
    }
    pub fn lifecycle_epoch(&self) -> u64 {
        self.lifecycle_epoch
    }
    pub fn access_generation(&self) -> u64 {
        self.access_generation
    }
    pub fn scopes(&self) -> &IdentityScopeSet {
        &self.scopes
    }
    pub fn auth_time(&self) -> u64 {
        self.auth_time
    }
    pub fn session_bounds(&self) -> CredentialBounds {
        CredentialBounds::new(vec![])
    }
    pub fn still_current(&self, current: &Self) -> bool {
        self == current
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessGeneration {
    pub generation: u64,
    pub eligible: bool,
}
impl AccessGeneration {
    /// Only final loss increments the generation. Regrant never revives old grants.
    pub fn transition(self, eligible: bool) -> Result<Self, IdentityError> {
        let generation = if self.eligible && !eligible {
            self.generation
                .checked_add(1)
                .ok_or(IdentityError::CapacityExceeded)?
        } else {
            self.generation
        };
        Ok(Self {
            generation,
            eligible,
        })
    }
}

pub fn evaluate_admission(input: &AdmissionInput<'_>) -> Result<ManagedAdmission, IdentityError> {
    ensure_enabled(input.account)?;
    input.root.validate(input.account, input.now_millis)?;
    let config_version_id = input.active_version_id.ok_or(IdentityError::AccessDenied)?;
    if !input.application_active
        || !input.owner_qualified
        || !input.client_enabled
        || !input.generation.eligible
        || !input.sources.eligible(input.environment_kind)
    {
        return Err(IdentityError::AccessDenied);
    }
    if !input.requested_scopes.is_subset_of(input.approved_scopes) {
        return Err(IdentityError::InvalidInput);
    }
    Ok(ManagedAdmission {
        principal: input.root.principal.clone(),
        sso_session_id: input.root.id,
        environment_id: input.environment_id,
        config_version_id,
        auth_epoch: input.account.auth_epoch,
        lifecycle_epoch: input.lifecycle_epoch,
        access_generation: input.generation.generation,
        scopes: input.requested_scopes.clone(),
        auth_time: input.root.authenticated_at / 1000,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::identity::account::{PersonKind, SecretDigest, stable_principal};

    fn account() -> AccountSnapshot {
        AccountSnapshot {
            user_id: 7,
            enabled: true,
            person_kind: PersonKind::Employee,
            developer_enabled: true,
            administrator: true,
            auth_epoch: 3,
        }
    }
    fn root() -> BrowserSessionSnapshot {
        BrowserSessionSnapshot {
            id: Uuid::new_v4(),
            principal: stable_principal("https://id.example", 7).unwrap(),
            user_id: 7,
            secret_digest: SecretDigest::of("root"),
            auth_epoch: 3,
            authenticated_at: 100_000,
            expires_at: 200_000,
            revoked_at: None,
        }
    }
    fn input<'a>(
        account: &'a AccountSnapshot,
        root: &'a BrowserSessionSnapshot,
        scopes: &'a IdentityScopeSet,
    ) -> AdmissionInput<'a> {
        AdmissionInput {
            account,
            root,
            now_millis: 110_000,
            environment_id: Uuid::new_v4(),
            environment_kind: AppEnvironmentKind::Production,
            application_active: true,
            owner_qualified: true,
            client_enabled: true,
            active_version_id: Some(Uuid::new_v4()),
            lifecycle_epoch: 2,
            generation: AccessGeneration {
                generation: 4,
                eligible: true,
            },
            sources: AdmissionSources {
                direct_assignment: true,
                ..Default::default()
            },
            approved_scopes: scopes,
            requested_scopes: scopes,
        }
    }

    #[test]
    fn test_admission_sources_are_environment_specific() {
        let a = account();
        let root = root();
        let scopes = IdentityScopeSet::parse("openid").unwrap();
        let mut i = input(&a, &root, &scopes);
        i.sources = AdmissionSources::default();
        assert_eq!(evaluate_admission(&i), Err(IdentityError::AccessDenied));
        i.sources.qualified_application_member = true;
        assert!(evaluate_admission(&i).is_err());
        i.environment_kind = AppEnvironmentKind::Development;
        assert!(evaluate_admission(&i).is_ok());
        i.sources = AdmissionSources {
            direct_assignment: true,
            ..Default::default()
        };
        assert!(evaluate_admission(&i).is_err());
        i.sources.explicit_test_user = true;
        assert!(evaluate_admission(&i).is_ok());
    }

    #[test]
    fn test_group_source_survives_direct_removal_and_final_loss_is_permanent() {
        let a = account();
        let root = root();
        let scopes = IdentityScopeSet::parse("openid").unwrap();
        let mut i = input(&a, &root, &scopes);
        let old = evaluate_admission(&i).unwrap();
        assert!(old.session_bounds().grants().is_empty());
        i.sources.direct_assignment = false;
        i.sources.group_assignments = 1;
        i.generation = i
            .generation
            .transition(i.sources.eligible(i.environment_kind))
            .unwrap();
        assert!(old.still_current(&evaluate_admission(&i).unwrap()));
        i.sources.group_assignments = 0;
        i.generation = i.generation.transition(false).unwrap();
        assert_eq!(i.generation.generation, 5);
        assert!(evaluate_admission(&i).is_err());
        i.sources.direct_assignment = true;
        i.generation = i.generation.transition(true).unwrap();
        assert_eq!(i.generation.generation, 5);
        assert!(!old.still_current(&evaluate_admission(&i).unwrap()));
        assert_eq!(
            AccessGeneration {
                generation: u64::MAX,
                eligible: true
            }
            .transition(false),
            Err(IdentityError::CapacityExceeded)
        );
    }

    #[test]
    fn test_admission_fails_closed_for_root_owner_app_config_and_epoch() {
        let a = account();
        let root = root();
        let scopes = IdentityScopeSet::parse("openid").unwrap();
        let i = input(&a, &root, &scopes);
        let old = evaluate_admission(&i).unwrap();
        assert_eq!(old.principal().subject(), "7");
        assert_eq!(old.auth_time(), 100);
        for field in 0..5 {
            let mut i = input(&a, &root, &scopes);
            match field {
                0 => i.application_active = false,
                1 => i.owner_qualified = false,
                2 => i.client_enabled = false,
                3 => i.active_version_id = None,
                _ => i.now_millis = root.expires_at,
            }
            assert!(evaluate_admission(&i).is_err());
        }
        let mut i = i;
        i.lifecycle_epoch += 1;
        assert!(!old.still_current(&evaluate_admission(&i).unwrap()));
    }
}
