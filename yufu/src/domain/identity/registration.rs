//! Validated self-service application registration; ownership comes from authentication.
use super::{application::*, error::IdentityError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateApplication {
    pub app_key: String,
    pub name: String,
    pub description: Option<String>,
    pub third_party: bool,
    pub development: EnvironmentConfigInput,
    pub production: EnvironmentConfigInput,
}

pub struct ValidatedRegistration {
    pub app_key: String,
    pub name: String,
    pub description: Option<String>,
    pub third_party: bool,
    pub development: EnvironmentConfig,
    pub production: EnvironmentConfig,
    pub request_hash: String,
}

impl CreateApplication {
    pub fn validate(self) -> Result<ValidatedRegistration, IdentityError> {
        self.validate_for_management(false)
    }
    pub(crate) fn validate_for_management(
        self,
        administrator: bool,
    ) -> Result<ValidatedRegistration, IdentityError> {
        auth::security::policy::ResourceRef::new(
            &self.app_key,
            auth::security::policy::ResourceTarget::Application,
        )?;
        let name = self.name.trim().to_owned();
        let description = self
            .description
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        if name.is_empty()
            || name.len() > 200
            || name.chars().any(char::is_control)
            || description
                .as_ref()
                .is_some_and(|s| s.len() > 2000 || s.chars().any(char::is_control))
        {
            return Err(IdentityError::InvalidInput);
        }
        let development = self
            .development
            .validate(AppEnvironmentKind::Development, true)?;
        let production = self
            .production
            .validate(AppEnvironmentKind::Production, administrator)?;
        if !administrator
            && (development.consent_policy != ConsentPolicy::UserConsent
                || production.consent_policy != ConsentPolicy::UserConsent)
        {
            return Err(IdentityError::Forbidden);
        }
        let canonical = CreateApplication {
            app_key: self.app_key.clone(),
            name: name.clone(),
            description: description.clone(),
            third_party: self.third_party,
            development: development.to_input(),
            production: production.to_input(),
        };
        let request_hash = hex::encode(Sha256::digest(
            serde_json::to_vec(&canonical).map_err(|_| IdentityError::InvalidInput)?,
        ));
        Ok(ValidatedRegistration {
            app_key: self.app_key,
            name,
            description,
            third_party: self.third_party,
            development,
            production,
            request_hash,
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateAdminApplication {
    pub application: CreateApplication,
    pub owner_user_id: i32,
    pub assignments: Vec<AssignmentTarget>,
    pub accept_logout_limitation: bool,
}
pub struct ValidatedAdminRegistration {
    pub application: ValidatedRegistration,
    pub owner_user_id: i32,
    pub assignments: Vec<AssignmentTarget>,
    pub accept_logout_limitation: bool,
}
impl CreateAdminApplication {
    pub fn validate(mut self) -> Result<ValidatedAdminRegistration, IdentityError> {
        if self.owner_user_id <= 0
            || self.assignments.len() > 100
            || self
                .assignments
                .iter()
                .any(|target| matches!(target,AssignmentTarget::User(id) if *id<=0))
        {
            return Err(IdentityError::InvalidInput);
        }
        let mut application = self.application.validate_for_management(true)?;
        if !self.accept_logout_limitation
            && [
                application.development.logout_capability,
                application.production.logout_capability,
            ]
            .contains(&LogoutCapability::LocalOnly)
        {
            return Err(IdentityError::InvalidInput);
        }
        self.assignments.sort();
        self.assignments.dedup();
        application.request_hash = hex::encode(Sha256::digest(
            serde_json::to_vec(&(
                application.request_hash.clone(),
                self.owner_user_id,
                &self.assignments,
                self.accept_logout_limitation,
            ))
            .map_err(|_| IdentityError::InvalidInput)?,
        ));
        Ok(ValidatedAdminRegistration {
            application,
            owner_user_id: self.owner_user_id,
            assignments: self.assignments,
            accept_logout_limitation: self.accept_logout_limitation,
        })
    }
}

pub fn validate_idempotency_key(key: &str) -> Result<(), IdentityError> {
    if key.is_empty()
        || key.len() > 128
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
    {
        return Err(IdentityError::InvalidInput);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> CreateApplication {
        let environment = EnvironmentConfigInput {
            client_kind: ClientKind::ServerWeb,
            redirect_uris: vec!["https://rp.example/callback".into()],
            post_logout_redirect_uris: vec![],
            browser_origins: vec![],
            access_audience: "urn:rp:api".into(),
            identity_scopes: "openid profile".into(),
            consent_policy: ConsentPolicy::UserConsent,
            logout_capability: LogoutCapability::Online,
            backchannel_logout_uri: None,
        };
        CreateApplication {
            app_key: "registration-test".into(),
            name: " Test application ".into(),
            description: None,
            third_party: true,
            development: environment.clone(),
            production: environment,
        }
    }
    #[test]
    fn registration_normalizes_name_and_scope_for_the_operation_hash() {
        let first = input().validate().unwrap();
        let mut next = input();
        next.name = "Test application".into();
        next.development.identity_scopes = "profile openid openid".into();
        assert_eq!(first.request_hash, next.validate().unwrap().request_hash);
        let mut next = input();
        next.production.access_audience = "urn:rp:other".into();
        assert_ne!(first.request_hash, next.validate().unwrap().request_hash);
    }
    #[test]
    fn registration_rejects_self_preconsent_and_requires_valid_development() {
        let mut request = input();
        request.production.consent_policy = ConsentPolicy::EnterprisePreconsent;
        assert_eq!(request.validate().err(), Some(IdentityError::Forbidden));
        let mut request = input();
        request.development.redirect_uris.clear();
        assert!(request.validate().is_err());
        let mut request = input();
        request.production.redirect_uris.clear();
        assert!(request.validate().is_ok());
    }
    #[test]
    fn registration_wire_cannot_choose_owner_or_management_state() {
        for field in [
            "owner_user_id",
            "management_mode",
            "management_state",
            "administrator",
        ] {
            let mut wire = serde_json::to_value(input()).unwrap();
            wire[field] = serde_json::json!(1);
            assert!(serde_json::from_value::<CreateApplication>(wire).is_err());
        }
        for key in ["", "a b", "abc/def", "\n", &"a".repeat(129)] {
            assert!(validate_idempotency_key(key).is_err());
        }
        assert!(validate_idempotency_key("create:123-abc").is_ok());
    }
}
