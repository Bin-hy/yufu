//! Explicit historical adoption. The request never supplies an issuer or secret.
use super::{
    application::AppEnvironmentKind,
    error::IdentityError,
    registration::{CreateAdminApplication, ValidatedAdminRegistration},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Historical payloads are conflict evidence only, never authentication proof.
/// Opaque legacy records and malformed JWTs do not assert an issuer.
pub(crate) fn historical_token_issuer(token: &str) -> Option<String> {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    if token.len() > 16_384 {
        return None;
    }
    let mut parts = token.split('.');
    let header = parts.next()?;
    let payload = parts.next()?;
    let signature = parts.next()?;
    if parts.next().is_some() || header.is_empty() || signature.is_empty() {
        return None;
    }
    let payload = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&payload).ok()?;
    let issuer = value.get("iss")?.as_str()?;
    if issuer.is_empty() || issuer.len() > 2048 || issuer.chars().any(char::is_control) {
        return None;
    }
    Some(issuer.to_owned())
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptLegacyApplication {
    pub oauth_client_id: i32,
    pub expected_snapshot: String,
    pub existing_application_id: Option<i32>,
    pub retained_environment: AppEnvironmentKind,
    pub registration: CreateAdminApplication,
}
pub struct ValidatedAdoption {
    pub oauth_client_id: i32,
    pub expected_snapshot: String,
    pub existing_application_id: Option<i32>,
    pub retained_environment: AppEnvironmentKind,
    pub registration: ValidatedAdminRegistration,
    pub request_hash: String,
}
impl AdoptLegacyApplication {
    pub fn validate(self) -> Result<ValidatedAdoption, IdentityError> {
        if self.oauth_client_id <= 0
            || self.existing_application_id.is_some_and(|id| id <= 0)
            || self.expected_snapshot.len() != 64
            || !self
                .expected_snapshot
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(IdentityError::InvalidInput);
        }
        let registration = self.registration.validate()?;
        let request_hash = hex::encode(Sha256::digest(
            serde_json::to_vec(&(
                self.oauth_client_id,
                &self.expected_snapshot,
                self.existing_application_id,
                self.retained_environment,
                &registration.application.request_hash,
            ))
            .map_err(|_| IdentityError::InvalidInput)?,
        ));
        Ok(ValidatedAdoption {
            oauth_client_id: self.oauth_client_id,
            expected_snapshot: self.expected_snapshot,
            existing_application_id: self.existing_application_id,
            retained_environment: self.retained_environment,
            registration,
            request_hash,
        })
    }
}

#[derive(Clone, Serialize)]
pub struct LegacyClientSummary {
    pub oauth_client_id: i32,
    pub client_id: String,
    pub name: String,
    pub candidate_owner_id: i32,
    pub public_client: bool,
    pub management_state: String,
    pub candidate_application_ids: Vec<i32>,
    pub conflict_kinds: Vec<String>,
    pub snapshot: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::identity::application::*;
    #[test]
    fn historical_issuer_only_reads_bounded_explicit_conflict_evidence() {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        let token = format!(
            "header.{}.signature",
            URL_SAFE_NO_PAD.encode(br#"{"iss":"https://old.example"}"#)
        );
        assert_eq!(
            historical_token_issuer(&token).as_deref(),
            Some("https://old.example")
        );
        for bad in [
            "opaque",
            "a.!!.c",
            "a.e30.c",
            "a.e30.",
            "a.e30.c.extra",
            &"a".repeat(16_385),
        ] {
            assert!(historical_token_issuer(bad).is_none());
        }
    }
    fn request() -> AdoptLegacyApplication {
        let environment = EnvironmentConfigInput {
            client_kind: ClientKind::ServerWeb,
            redirect_uris: vec!["https://rp.example/callback".into()],
            post_logout_redirect_uris: vec![],
            browser_origins: vec![],
            access_audience: "urn:rp:api".into(),
            identity_scopes: "openid".into(),
            consent_policy: ConsentPolicy::UserConsent,
            logout_capability: LogoutCapability::Online,
            backchannel_logout_uri: None,
            launch_url: None,
            launch_description: None,
        };
        AdoptLegacyApplication {
            oauth_client_id: 1,
            expected_snapshot: "a".repeat(64),
            existing_application_id: None,
            retained_environment: AppEnvironmentKind::Production,
            registration: CreateAdminApplication {
                application: super::super::registration::CreateApplication {
                    app_key: "legacy-app".into(),
                    name: "Legacy App".into(),
                    description: None,
                    third_party: true,
                    development: environment.clone(),
                    production: environment,
                },
                owner_user_id: 2,
                assignments: vec![AssignmentTarget::User(2)],
                accept_logout_limitation: false,
            },
        }
    }
    #[test]
    fn adoption_binds_snapshot_environment_owner_and_application() {
        let input = request();
        let original = input.clone().validate().unwrap().request_hash;
        let mut changed = input.clone();
        changed.retained_environment = AppEnvironmentKind::Development;
        assert_ne!(original, changed.validate().unwrap().request_hash);
        let mut changed = input.clone();
        changed.registration.owner_user_id = 3;
        assert_ne!(original, changed.validate().unwrap().request_hash);
        let mut changed = input.clone();
        changed.existing_application_id = Some(4);
        assert_ne!(original, changed.validate().unwrap().request_hash);
        let mut changed = input;
        changed.expected_snapshot = "b".repeat(64);
        assert_ne!(original, changed.validate().unwrap().request_hash);
    }
    #[test]
    fn adoption_rejects_unbounded_or_ambiguous_inputs() {
        let mut input = request();
        input.expected_snapshot = "secret".into();
        assert!(matches!(input.validate(), Err(IdentityError::InvalidInput)));
        let mut input = request();
        input.existing_application_id = Some(0);
        assert!(matches!(input.validate(), Err(IdentityError::InvalidInput)));
        let mut input = request();
        input.registration.owner_user_id = 0;
        assert!(matches!(input.validate(), Err(IdentityError::InvalidInput)));
        let mut input = serde_json::to_value(request()).unwrap();
        input["issuer"] = serde_json::json!("https://other.example");
        assert!(serde_json::from_value::<AdoptLegacyApplication>(input).is_err());
    }
}
