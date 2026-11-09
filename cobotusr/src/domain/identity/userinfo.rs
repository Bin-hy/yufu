use super::application::IdentityScopeSet;
use auth::claims::IdentityProfile;
use serde::Serialize;

#[derive(Serialize)]
pub struct UserInfo {
    pub sub: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
}
impl UserInfo {
    pub fn from_profile(sub: String, scopes: &IdentityScopeSet, profile: IdentityProfile) -> Self {
        let disclose_profile = scopes.contains("profile");
        let disclose_email = scopes.contains("email");
        let email = disclose_email.then_some(profile.email).flatten();
        let email_verified = if email.is_some() {
            profile.email_verified
        } else {
            None
        };
        Self {
            sub,
            name: disclose_profile.then_some(profile.name).flatten(),
            preferred_username: disclose_profile
                .then_some(profile.preferred_username)
                .flatten(),
            email,
            email_verified,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_userinfo_omits_unapproved_and_absent_profile_fields() {
        let profile = IdentityProfile {
            name: Some("Name".into()),
            preferred_username: Some("username".into()),
            email: Some("real@example.test".into()),
            email_verified: Some(false),
        };
        let minimal = UserInfo::from_profile(
            "17".into(),
            &IdentityScopeSet::parse("openid").unwrap(),
            profile.clone(),
        );
        assert_eq!(
            serde_json::to_value(minimal).unwrap(),
            serde_json::json!({"sub":"17"})
        );
        let email = UserInfo::from_profile(
            "17".into(),
            &IdentityScopeSet::parse("openid email").unwrap(),
            profile,
        );
        assert_eq!(
            serde_json::to_value(email).unwrap(),
            serde_json::json!({"sub":"17","email":"real@example.test","email_verified":false})
        );
        let absent = UserInfo::from_profile(
            "17".into(),
            &IdentityScopeSet::parse("openid profile email").unwrap(),
            IdentityProfile {
                email_verified: Some(true),
                ..Default::default()
            },
        );
        assert_eq!(
            serde_json::to_value(absent).unwrap(),
            serde_json::json!({"sub":"17"})
        );
    }
}
